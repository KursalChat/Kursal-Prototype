use crate::{
    Result,
    api::{AppEvent, ConnectionStatus, CoreCommand, handle_core_command, handle_incoming},
    contacts::Contact,
    first_contact::nearby::{
        MdnsTransport, NearbyBeacon, NearbyRouteResult, NearbyTransport, handle_nearby_request,
    },
    identity::init_transport,
    network::{
        rendezvous::lookup_rendezvous,
        swarm::{ConnectionKind, NetworkEvent, SwarmHandle},
    },
    storage::{Database, SharedDatabase, relay_server_enabled},
};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc, oneshot};

pub const BOOTSTRAP_PEERS: &[&str] = &[
    // "/dnsaddr/bootstrap.libp2p.io/p2p/QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN",
    // "/ip4/192.168.178.9/udp/4891/quic-v1/p2p/12D3KooW9sfuTYevisKu5JV9LWrMmaSYu8SnZnG8vEPPW7UFrXJX",
    "/dns4/diffie.kursal.chat/udp/4891/quic-v1/p2p/12D3KooW9sfuTYevisKu5JV9LWrMmaSYu8SnZnG8vEPPW7UFrXJX",
];

pub mod dht;
pub mod kademlia;
pub mod rendezvous;
pub mod rotation;
pub mod swarm;

pub struct NetworkManager {
    pub primary: SwarmHandle,
    pub secondary: Option<SwarmHandle>,
    pub event_tx: mpsc::Sender<NetworkEvent>,
    pub mdns_transport: Option<Arc<MdnsTransport>>,
}

impl NetworkManager {
    pub async fn new(db: &Database) -> Result<(Self, mpsc::Receiver<NetworkEvent>)> {
        let (event_tx, event_rx) = mpsc::channel(64);
        let identity = init_transport(db)?;
        let primary =
            SwarmHandle::spawn(identity, event_tx.clone(), relay_server_enabled(db)?).await?;

        let mdns_transport = MdnsTransport::new(primary.cmd_tx.clone());

        Ok((
            Self {
                primary,
                secondary: None,
                event_tx,
                mdns_transport: Some(Arc::new(mdns_transport)),
            },
            event_rx,
        ))
    }

    pub async fn start_mdns(&mut self, beacon: NearbyBeacon) -> Result<()> {
        if let Some(transport) = &self.mdns_transport {
            transport.start(beacon).await;
        }

        Ok(())
    }

    pub async fn stop_mdns(&mut self) -> Result<()> {
        if let Some(transport) = &self.mdns_transport {
            transport.stop().await;
        }

        Ok(())
    }
}

pub async fn get_nearby_peers(network: &NetworkManager) -> Vec<(String, NearbyBeacon)> {
    match network.mdns_transport.as_ref() {
        Some(t) => t.nearby_peers_snapshot().await,
        None => Vec::new(),
    }
}

pub async fn dispatch_events(
    mut event_rx: mpsc::Receiver<NetworkEvent>,
    mut core_cmd_rx: mpsc::Receiver<CoreCommand>,
    db: SharedDatabase,
    network: Arc<Mutex<NetworkManager>>,
    app_event_tx: mpsc::Sender<AppEvent>,
) {
    loop {
        tokio::select! {
            Some(event) = event_rx.recv() => {
                handle_internal_network_event(event, &db, &network, &app_event_tx).await
            }
            Some(cmd) = core_cmd_rx.recv() => {
                let db = db.clone();
                let network = network.clone();
                let app_event_tx = app_event_tx.clone();
                tokio::task::spawn_local(async move {
                    handle_core_command(cmd, db, network, app_event_tx).await;
                });
            }
            else => break
        }
    }
}

async fn handle_internal_network_event(
    event: NetworkEvent,
    db: &SharedDatabase,
    network: &Arc<Mutex<NetworkManager>>,
    app_event_tx: &mpsc::Sender<AppEvent>,
) {
    match event {
        NetworkEvent::PeerDiscovered { peer_id, .. } => {
            if let Some(transport) = &network.lock().await.mdns_transport {
                let _ = transport.on_peer_discovered(peer_id.to_base58()).await;
            }
        }

        NetworkEvent::MessageReceived { from, data } => {
            let mdns_transport = network.lock().await.mdns_transport.clone();
            let route = match &mdns_transport {
                Some(t) => t.on_message_received(from.to_base58(), data.clone()).await,
                None => NearbyRouteResult::NotNearby,
            };

            match route {
                NearbyRouteResult::NotNearby => {
                    let cmd_tx = network.lock().await.primary.cmd_tx.clone();
                    let db_clone = db.clone();
                    let app_event_tx_clone = app_event_tx.clone();

                    tokio::task::spawn_local(async move {
                        if let Err(e) =
                            handle_incoming(from, data, db_clone, &cmd_tx, &app_event_tx_clone)
                                .await
                        {
                            log::warn!("handle_incoming error: {e}");
                        }
                    });
                }
                NearbyRouteResult::HandledInternally => { /* ignore */ }
                NearbyRouteResult::IncomingRequest {
                    peer_id,
                    session_name,
                } => {
                    let (decision_tx, decision_rx) = oneshot::channel::<bool>();

                    if app_event_tx
                        .send(AppEvent::NearbyRequest {
                            peer_id: peer_id.clone(),
                            session_name,
                            decision_tx,
                        })
                        .await
                        .is_err()
                    {
                        // receiver dropped, skip handling
                        return;
                    }

                    if let Some(transport) = mdns_transport {
                        let db_clone = db.clone();
                        let event_tx_clone = app_event_tx.clone();
                        let cmd_tx = network.lock().await.primary.cmd_tx.clone();

                        tokio::task::spawn_local(async move {
                            if let Err(e) = handle_nearby_request(
                                &peer_id,
                                transport.as_ref(),
                                decision_rx,
                                db_clone,
                                &event_tx_clone,
                                &cmd_tx,
                            )
                            .await
                            {
                                log::warn!("handle_nearby_request error: {e}");
                            }
                        });
                    }
                }
            }
        }

        NetworkEvent::ConnectionEstablished { peer_id, via } => {
            log::info!("[network] connection establoished with {peer_id} via {via:?}");
            let peer_id_str = peer_id.to_base58();

            if let Ok(contacts) = Contact::load_all(&*db.0.lock().await)
                && let Some(contact) = contacts.iter().find(|c| c.peer_id == peer_id_str)
            {
                let status = match via {
                    ConnectionKind::Relay => ConnectionStatus::Relay,
                    ConnectionKind::Direct => ConnectionStatus::Direct,
                    ConnectionKind::HolePunch => ConnectionStatus::HolePunch,
                };

                app_event_tx
                    .send(AppEvent::ConnectionChange {
                        contact_id: contact.user_id.clone(),
                        status,
                    })
                    .await
                    .ok();
            }
        }
        NetworkEvent::ConnectionLost { peer_id } => {
            let peer_id_str = peer_id.to_base58();

            if let Ok(contacts) = Contact::load_all(&*db.0.lock().await)
                && let Some(contact) = contacts.iter().find(|c| c.peer_id == peer_id_str)
            {
                app_event_tx
                    .send(AppEvent::ConnectionChange {
                        contact_id: contact.user_id.clone(),
                        status: ConnectionStatus::Disconnected,
                    })
                    .await
                    .ok();
            }
        }

        NetworkEvent::DhtFetchResult { .. } => { /* nothing happens here */ }

        NetworkEvent::SendFailed { peer_id } => {
            let peer_id_str = peer_id.to_base58();
            let db_lock = db.0.lock().await;

            let Ok(contacts) = Contact::load_all(&db_lock) else {
                return;
            };
            let Some(mut contact) = contacts.into_iter().find(|c| c.peer_id == peer_id_str) else {
                return;
            };
            drop(db_lock);

            let net = network.lock().await;

            match lookup_rendezvous(&contact.identity_pub_key, &contact.dilithium_pub_key, &net)
                .await
            {
                Ok(Some(record)) => {
                    contact.peer_id = record.peer_id;
                    contact.known_addresses = record.relay_addresses;
                    contact.save(&*db.0.lock().await).ok();
                }
                _ => {
                    app_event_tx
                        .send(AppEvent::ConnectionChange {
                            contact_id: contact.user_id,
                            status: ConnectionStatus::Disconnected,
                        })
                        .await
                        .ok();
                }
            }
        }
    }
}
