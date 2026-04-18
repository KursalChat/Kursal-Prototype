use crate::{
    KursalError, Result,
    api::{AppEvent, ConnectionStatus, CoreCommand, handle_core_command, handle_incoming},
    contacts::Contact,
    first_contact::nearby::{
        BtEvent, NearbyBeacon, NearbyMessage, NearbyOrigin, NearbyPacket, NearbyRouteResult,
        NearbyTransport, bluetooth::BTTransport, handle_nearby_request, mdns::MdnsTransport,
    },
    identity::init_transport,
    network::{
        rendezvous::lookup_rendezvous,
        swarm::{ConnectionKind, NetworkEvent, SwarmCommand, SwarmHandle},
    },
    storage::{Database, SharedDatabase, relay_server_enabled},
};
use libp2p::PeerId;
use std::{collections::HashMap, str::FromStr, sync::Arc};
use tokio::sync::{Mutex, mpsc, oneshot};

pub const BOOTSTRAP_PEERS: &[&str] = &[
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
    pub bt_event_tx: mpsc::Sender<BtEvent>,
    //
    pub my_beacon: Arc<Mutex<Option<NearbyBeacon>>>,
    pub nearby_peers: Arc<Mutex<HashMap<(String, NearbyOrigin), NearbyBeacon>>>,
    pub mdns_transport: Arc<MdnsTransport>,
    pub bt_transport: Arc<BTTransport>,
}

impl NetworkManager {
    pub async fn new(
        db: &Database,
    ) -> Result<(Self, mpsc::Receiver<NetworkEvent>, mpsc::Receiver<BtEvent>)> {
        let (event_tx, event_rx) = mpsc::channel(64);
        let (bt_event_tx, bt_event_rx) = mpsc::channel(64);
        let identity = init_transport(db)?;
        let primary =
            SwarmHandle::spawn(identity, event_tx.clone(), relay_server_enabled(db)?).await?;

        let my_beacon = Arc::new(Mutex::new(None));
        let mdns_transport = MdnsTransport::new(primary.cmd_tx.clone(), my_beacon.clone());
        let bt_transport = BTTransport::new(
            primary.cmd_tx.clone(),
            my_beacon.clone(),
            bt_event_tx.clone(),
        );

        Ok((
            Self {
                primary,
                secondary: None,
                event_tx,
                bt_event_tx,
                //
                my_beacon,
                nearby_peers: Arc::new(Mutex::new(HashMap::new())),
                mdns_transport: Arc::new(mdns_transport),
                bt_transport: Arc::new(bt_transport),
            },
            event_rx,
            bt_event_rx,
        ))
    }

    pub async fn start_mdns(&mut self, beacon: NearbyBeacon) -> Result<()> {
        *self.my_beacon.lock().await = Some(beacon.clone());

        if let Err(e) = self.primary.cmd_tx.send(SwarmCommand::EnableNearby).await {
            log::error!("Failed to enable mDNS: {e}");
        }

        self.mdns_transport.start(beacon.clone()).await;
        self.bt_transport.start(beacon).await;

        Ok(())
    }

    pub async fn stop_mdns(&mut self) -> Result<()> {
        self.nearby_peers.lock().await.clear();
        *self.my_beacon.lock().await = None;

        if let Err(e) = self.primary.cmd_tx.send(SwarmCommand::DisableNearby).await {
            log::error!("Failed to disable mDNS: {e}");
        }

        self.mdns_transport.stop().await;
        self.bt_transport.stop().await;

        Ok(())
    }
}

pub async fn get_nearby_peers(
    network: &NetworkManager,
) -> Vec<(String, NearbyBeacon, NearbyOrigin)> {
    network
        .nearby_peers
        .lock()
        .await
        .clone()
        .into_iter()
        .map(|((peer_id, origin), beacon)| (peer_id, beacon, origin))
        .collect()
}

pub async fn dispatch_events(
    mut event_rx: mpsc::Receiver<NetworkEvent>,
    mut bt_event_rx: mpsc::Receiver<BtEvent>,
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
            Some(event) = bt_event_rx.recv() => {
                handle_bt_event(event, &db, &network, &app_event_tx).await
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

async fn handle_bt_event(
    event: BtEvent,
    db: &SharedDatabase,
    network: &Arc<Mutex<NetworkManager>>,
    app_event_tx: &mpsc::Sender<AppEvent>,
) {
    match event {
        BtEvent::Beacon { peer_id, beacon } => {
            let net = network.lock().await;

            log::info!("[nearby] Discovered {peer_id} via bluetooth");
            net.nearby_peers
                .lock()
                .await
                .insert((peer_id, NearbyOrigin::Bluetooth), beacon);
        }
        BtEvent::Message { from_peer_id, msg } => {
            let bt_transport = network.lock().await.bt_transport.clone();

            let sender = bt_transport
                .pending_handshakes
                .lock()
                .await
                .get(&from_peer_id)
                .cloned();

            match sender {
                Some(tx) => {
                    let _ = tx.send(msg).await;
                }
                None => {
                    if let NearbyMessage::ConnectRequest { from_session_name } = msg {
                        let (decision_tx, decision_rx) = oneshot::channel::<bool>();
                        if app_event_tx
                            .send(AppEvent::NearbyRequest {
                                peer_id: from_peer_id.clone(),
                                session_name: from_session_name,
                                decision_tx,
                            })
                            .await
                            .is_err()
                        {
                            return;
                        }

                        let db_clone = db.clone();
                        let event_tx_clone = app_event_tx.clone();
                        let cmd_tx = network.lock().await.primary.cmd_tx.clone();
                        let bt_arc = bt_transport.clone();

                        tokio::task::spawn_local(async move {
                            if let Err(e) = handle_nearby_request(
                                &from_peer_id,
                                &*bt_arc,
                                decision_rx,
                                db_clone,
                                &event_tx_clone,
                                &cmd_tx,
                            )
                            .await
                            {
                                log::warn!("[bt] handle_nearby_request: {e}");
                            }
                        });
                    }
                }
            }
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
            let network = network.lock().await;

            let result: Result<()> = async {
                let beacon = network
                    .my_beacon
                    .lock()
                    .await
                    .clone()
                    .ok_or(KursalError::Storage("No beacon saved".to_string()))?;

                let serialized = NearbyPacket::Beacon(beacon).serialize()?;

                network
                    .primary
                    .cmd_tx
                    .send(SwarmCommand::SendMessage {
                        peer_id: PeerId::from_str(&peer_id.to_base58())
                            .map_err(|err| KursalError::Crypto(err.to_string()))?,
                        data: serialized,
                        addresses: Vec::new(), // TODO: is there bettter?
                    })
                    .await
                    .map_err(|err| KursalError::Network(err.to_string()))?;

                Ok(())
            }
            .await;

            if let Err(err) = result {
                log::warn!("failed to send beacon to {peer_id}: {err}");
            }
        }

        NetworkEvent::MessageReceived { from, data } => {
            let route = async {
                let packet = match NearbyPacket::deserialize(&data) {
                    Ok(p) => p,
                    Err(_) => return NearbyRouteResult::NotNearby,
                };

                match packet {
                    NearbyPacket::Beacon(mut b) => {
                        // imagine a guy just putting another peer id 💀
                        b.peer_id = from.to_base58();

                        let network = network.lock().await;

                        log::info!("[nearby] Discovered {} via mDNS", b.peer_id);
                        network
                            .nearby_peers
                            .lock()
                            .await
                            .insert((b.peer_id.clone(), NearbyOrigin::mDNS), b);

                        if let Some(my_beacon) = network.my_beacon.lock().await.clone()
                            && let Ok(data) = NearbyPacket::BeaconAck(my_beacon).serialize()
                        {
                            let _ = network
                                .primary
                                .cmd_tx
                                .send(SwarmCommand::SendMessage {
                                    peer_id: from,
                                    data,
                                    addresses: Vec::new(),
                                })
                                .await;
                        }

                        NearbyRouteResult::HandledInternally
                    }
                    NearbyPacket::BeaconAck(mut b) => {
                        b.peer_id = from.to_base58();

                        let network = network.lock().await;

                        log::info!("[nearby] Ack from {} via mDNS", b.peer_id);
                        network
                            .nearby_peers
                            .lock()
                            .await
                            .insert((b.peer_id.clone(), NearbyOrigin::mDNS), b);

                        NearbyRouteResult::HandledInternally
                    }
                    NearbyPacket::Message(msg) => {
                        let sender = {
                            let network = network.lock().await;
                            network
                                .mdns_transport
                                .pending_handshakes
                                .lock()
                                .await
                                .get(&from.to_base58())
                                .cloned()
                        };
                        match sender {
                            Some(tx) => {
                                let _ = tx.send(msg).await;
                                NearbyRouteResult::HandledInternally
                            }
                            None => match msg {
                                NearbyMessage::ConnectRequest { from_session_name } => {
                                    NearbyRouteResult::IncomingRequest {
                                        peer_id: from.to_base58(),
                                        session_name: from_session_name,
                                    }
                                }
                                _ => {
                                    // invalid
                                    NearbyRouteResult::HandledInternally
                                }
                            },
                        }
                    }
                }
            }
            .await;

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

                    let db_clone = db.clone();
                    let event_tx_clone = app_event_tx.clone();
                    let cmd_tx = network.lock().await.primary.cmd_tx.clone();

                    let mdns_transport = network.lock().await.mdns_transport.clone();
                    tokio::task::spawn_local(async move {
                        if let Err(e) = handle_nearby_request(
                            &peer_id,
                            &*mdns_transport,
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

            log::info!("send to {peer_id} failed, trying lookup_rendezvous");
            match lookup_rendezvous(&contact.identity_pub_key, &contact.dilithium_pub_key, &net)
                .await
            {
                Ok(Some(record)) => {
                    log::info!(
                        "lookup_rendezvous discovered new addresses for peer {peer_id} -> {}",
                        record.peer_id
                    );

                    contact.peer_id = record.peer_id;
                    contact.known_addresses = record.relay_addresses;
                    contact.save(&*db.0.lock().await).ok();

                    app_event_tx
                        .send(AppEvent::ConnectionChange {
                            contact_id: contact.user_id,
                            status: ConnectionStatus::Disconnected,
                        })
                        .await
                        .ok();
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
