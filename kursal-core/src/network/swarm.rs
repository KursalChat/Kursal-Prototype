use crate::{
    KursalError, Result,
    contacts::Contact,
    identity::TransportIdentity,
    network::{
        BOOTSTRAP_PEERS,
        kademlia::{KAD_MAX_PAYLOAD, KursalKadStore},
    },
};
use futures::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use libp2p::{
    Multiaddr, PeerId, StreamProtocol, Swarm, SwarmBuilder,
    futures::StreamExt,
    mdns,
    multiaddr::Protocol,
    request_response::{self, ProtocolSupport},
    swarm::{NetworkBehaviour, SwarmEvent, behaviour::toggle::Toggle},
};
use std::collections::{HashMap, HashSet};
use std::io;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

pub const MAX_MESSAGE_SIZE: usize = 512 * 1024; // 512 KB, should LARGE be enough

pub enum SwarmCommand {
    Dial(Multiaddr),
    SendMessage {
        peer_id: PeerId,
        data: Vec<u8>,
        addresses: Vec<Multiaddr>,
    },
    PublishDht {
        key: Vec<u8>,
        value: Vec<u8>,
    },
    FetchDht {
        key: Vec<u8>,
        reply_tx: mpsc::Sender<Vec<u8>>,
    },
    Shutdown,
    EnableMdns,
    DisableMdns,
    ContactAdded {
        contact: Contact,
    },
    GetListenAddresses {
        reply_tx: oneshot::Sender<Vec<Multiaddr>>,
    },
}

#[derive(Debug, PartialEq)]
pub enum ConnectionKind {
    Relay,
    HolePunch,
    Direct,
}

pub enum NetworkEvent {
    MessageReceived {
        from: PeerId,
        data: Vec<u8>,
    },
    PeerDiscovered {
        peer_id: PeerId,
        addresses: Vec<Multiaddr>,
    },
    ConnectionEstablished {
        peer_id: PeerId,
        via: ConnectionKind,
    },
    ConnectionLost {
        peer_id: PeerId,
    },
    // DhtPublishOk {
    //     key: Vec<u8>,
    // },
    DhtFetchResult {
        key: Vec<u8>,
        value: Option<Vec<u8>>,
    },
    SendFailed {
        peer_id: PeerId,
    },
}

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "KursalBehaviourEvent")]
pub struct KursalBehaviour {
    pub relay: libp2p::relay::client::Behaviour,
    pub relay_server: Toggle<libp2p::relay::Behaviour>,
    pub dcutr: libp2p::dcutr::Behaviour,
    pub kad: libp2p::kad::Behaviour<KursalKadStore>,
    pub mdns: libp2p::mdns::tokio::Behaviour,
    pub identify: libp2p::identify::Behaviour,
    pub request_response: request_response::Behaviour<KursalMsgCodec>,
}

pub struct SwarmHandle {
    pub peer_id: PeerId,
    pub cmd_tx: mpsc::Sender<SwarmCommand>,
    pub relay_server_enabled: bool,
}

impl SwarmHandle {
    pub async fn spawn(
        identity: TransportIdentity,
        event_tx: mpsc::Sender<NetworkEvent>,
        relay_server_enabled: bool,
    ) -> Result<Self> {
        let swarm = SwarmBuilder::with_existing_identity(identity.keypair)
            .with_tokio()
            .with_tcp(
                Default::default(),
                libp2p::noise::Config::new,
                libp2p::yamux::Config::default,
            )
            .map_err(|err| KursalError::Network(format!("swarm tcp error: {err}")))?
            .with_quic();

        #[cfg(any(target_os = "android", target_os = "ios"))]
        // TODO: maybe chance cloudflare to another DNS
        let swarm = swarm.with_dns_config(
            libp2p::dns::ResolverConfig::cloudflare(),
            libp2p::dns::ResolverOpts::default(),
        );

        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let swarm = swarm
            .with_dns()
            .map_err(|err| KursalError::Network(format!("swarm dns error: {err}")))?;

        let mut swarm = swarm
            .with_relay_client(libp2p::noise::Config::new, libp2p::yamux::Config::default)
            .map_err(|err| KursalError::Network(format!("swarm relay error: {err}")))?
            .with_behaviour(|key, relay_client|  -> std::result::Result<_, Box<dyn std::error::Error + Send + Sync>> {
                let local_peer_id = key.public().to_peer_id();

                let relay = relay_client;
                let dcutr = libp2p::dcutr::Behaviour::new(local_peer_id);

                let mut kad_config = libp2p::kad::Config::new(StreamProtocol::new("/kursal/kad/1.0.0"));
                kad_config.set_max_packet_size(KAD_MAX_PAYLOAD);

                let kad = libp2p::kad::Behaviour::with_config(
                    local_peer_id,
                    KursalKadStore::new(local_peer_id),
                    kad_config,
                );

                let mdns = libp2p::mdns::tokio::Behaviour::new(mdns::Config {
                    query_interval: Duration::from_secs(20),
                    ttl: Duration::from_secs(90),
                    enable_ipv6: false,
                }, local_peer_id)?;

                let identify = libp2p::identify::Behaviour::new(libp2p::identify::Config::new(
                    "/kursal/v1.0.0".to_string(),
                    key.public(),
                ));

                let request_response = request_response::Behaviour::new(
                    [(StreamProtocol::new("/kursal/msg/1.0.0"), ProtocolSupport::Full)],
                    request_response::Config::default(),
                );

                let relay_server = if relay_server_enabled {
                    Toggle::from(Some(libp2p::relay::Behaviour::new(local_peer_id, Default::default())))
                } else {
                    Toggle::from(None)
                };

                Ok(KursalBehaviour {
                    relay,
                    relay_server,
                    dcutr,
                    kad,
                    mdns,
                    identify,
                    request_response
                })
            })
            .map_err(|err| KursalError::Network(format!("swarm behaviour error: {err}")))?
            .build();

        // TODO: maybe specify port?
        swarm
            .listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap())
            .map_err(|err| KursalError::Network(format!("swarm listen error: {err}")))?;
        swarm
            .listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse().unwrap())
            .map_err(|err| KursalError::Network(format!("swarm listen error: {err}")))?;

        let (cmd_tx, mut cmd_rx) = mpsc::channel::<SwarmCommand>(32);
        let peer_id = identity.peer_id;

        tokio::spawn(async move {
            log::info!("[swarm] event loop started");
            let mut pending_queries: HashMap<libp2p::kad::QueryId, mpsc::Sender<Vec<u8>>> =
                HashMap::new();
            let mut listen_addresses: HashSet<Multiaddr> = HashSet::new();
            let mut mdns_enabled = false;
            let mut mdns_peer_buffer: Vec<(PeerId, Multiaddr)> = Vec::new();

            for addr_str in BOOTSTRAP_PEERS {
                if let Ok(multiaddr) = addr_str.parse::<Multiaddr>()
                    && let Some(Protocol::P2p(peer_id)) = multiaddr.iter().last()
                {
                    swarm
                        .behaviour_mut()
                        .kad
                        .add_address(&peer_id, multiaddr.clone());
                    if let Err(err) = swarm.dial(multiaddr) {
                        log::warn!("Bootstrap peer {addr_str} could not be dialed: {err:?}");
                    } else {
                        log::info!("[bootstrap] dialing {addr_str}");
                    }
                }
            }

            loop {
                tokio::select! {
                    event = swarm.select_next_some() => handle_swarm_event(event, &event_tx, &mut pending_queries, &mut listen_addresses, &mut swarm, mdns_enabled, &mut mdns_peer_buffer).await,
                    cmd = cmd_rx.recv() => {
                        match cmd {
                            Some(SwarmCommand::Shutdown) => break,
                            Some(SwarmCommand::EnableMdns) => {
                                mdns_enabled = true;
                                for (peer_id, addr) in mdns_peer_buffer.drain(..) {
                                    let _ = event_tx.send(NetworkEvent::PeerDiscovered { peer_id, addresses: vec![addr] }).await;
                                }

                                log::info!("mDNS enabled");
                            },
                            Some(cmd) => handle_swarm_command(cmd, &mut swarm, &mut pending_queries, &mut listen_addresses, &mut mdns_enabled),
                            None => break
                        }
                    }
                }
            }
        });

        Ok(SwarmHandle {
            peer_id,
            cmd_tx,
            relay_server_enabled,
        })
    }
}

async fn handle_swarm_event(
    event: SwarmEvent<KursalBehaviourEvent>,
    event_tx: &mpsc::Sender<NetworkEvent>,
    pending_queries: &mut HashMap<libp2p::kad::QueryId, mpsc::Sender<Vec<u8>>>,
    listen_addresses: &mut HashSet<Multiaddr>,
    swarm: &mut Swarm<KursalBehaviour>,
    mdns_enabled: bool,
    mdns_peer_buffer: &mut Vec<(PeerId, Multiaddr)>,
) {
    match event {
        SwarmEvent::Behaviour(KursalBehaviourEvent::Kad(
            libp2p::kad::Event::OutboundQueryProgressed { id, result, .. },
        )) => match result {
            libp2p::kad::QueryResult::GetRecord(Ok(libp2p::kad::GetRecordOk::FoundRecord(
                peer_record,
            ))) => {
                if let Some(tx) = pending_queries.get(&id) {
                    // TODO: remove records when expired, should wait for all queries to arrive
                    let _ = tx.send(peer_record.record.value).await;
                }
            }
            libp2p::kad::QueryResult::GetRecord(Ok(
                libp2p::kad::GetRecordOk::FinishedWithNoAdditionalRecord { .. },
            )) => {
                log::debug!(
                    "[kad] GET record query={:?} finished with no additional record",
                    id
                );
                pending_queries.remove(&id);
            }
            libp2p::kad::QueryResult::GetRecord(Err(e)) => {
                log::warn!("[kad] GET record failed query={:?} error={:?}", id, e);
                pending_queries.remove(&id);
            }
            libp2p::kad::QueryResult::PutRecord(Ok(_)) => {
                log::info!("[kad] PUT record succeeded query={:?}", id);
            }
            libp2p::kad::QueryResult::PutRecord(Err(e)) => {
                log::warn!("[kad] PUT record failed query={:?} error={:?}", id, e);
            }
            _ => {}
        },

        SwarmEvent::Behaviour(KursalBehaviourEvent::Identify(
            libp2p::identify::Event::Received { peer_id, info, .. },
        )) => {
            log::debug!(
                "[identify] received from peer {}: protocols={:?}, listen_addrs={:?}",
                peer_id,
                info.protocols,
                info.listen_addrs
            );

            for addr in info.listen_addrs {
                swarm.behaviour_mut().kad.add_address(&peer_id, addr);
            }
        }
        SwarmEvent::Behaviour(KursalBehaviourEvent::Mdns(libp2p::mdns::Event::Discovered(
            peers,
        ))) => {
            log::info!(
                "[mDNS] raw Discovered event, mdns_enabled={}, peers={}",
                mdns_enabled,
                peers.len()
            );

            for (peer_id, addr) in peers {
                log::info!("[mDNS] discovered peer {} at {}", peer_id, addr);

                if mdns_enabled {
                    let _ = event_tx
                        .send(NetworkEvent::PeerDiscovered {
                            peer_id,
                            addresses: vec![addr],
                        })
                        .await;
                } else {
                    mdns_peer_buffer.push((peer_id, addr));
                }
            }
        }

        // TODO: add a PeerExpired or similar, connection is not lost
        SwarmEvent::Behaviour(KursalBehaviourEvent::Mdns(mdns::Event::Expired(peers))) => {
            for (peer_id, addr) in &peers {
                log::warn!("[mDNS] peer expired {} at {}", peer_id, addr);
            }
        }

        SwarmEvent::Behaviour(KursalBehaviourEvent::Dcutr(e)) => {
            if let Ok(_connection_id) = e.result {
                let _ = event_tx
                    .send(NetworkEvent::ConnectionEstablished {
                        peer_id: e.remote_peer_id,
                        via: ConnectionKind::HolePunch,
                    })
                    .await;
            }
        }

        SwarmEvent::Behaviour(KursalBehaviourEvent::Relay(
            libp2p::relay::client::Event::ReservationReqAccepted { relay_peer_id, .. },
        )) => {
            log::info!("[relay] reservation accepted by {relay_peer_id}");
        }
        SwarmEvent::Behaviour(KursalBehaviourEvent::Relay(
            libp2p::relay::client::Event::OutboundCircuitEstablished { relay_peer_id, .. },
        )) => {
            log::info!("[relay] outbound circuit established via {relay_peer_id}");
        }
        SwarmEvent::Behaviour(KursalBehaviourEvent::Relay(
            libp2p::relay::client::Event::InboundCircuitEstablished { src_peer_id, .. },
        )) => {
            log::info!("[relay] inbound circuit from {src_peer_id}");
        }

        SwarmEvent::Behaviour(KursalBehaviourEvent::RelayServer(
            libp2p::relay::Event::ReservationReqAccepted { src_peer_id, .. },
        )) => {
            log::info!("[relay] reservation accepted from {src_peer_id}");
        }
        SwarmEvent::Behaviour(KursalBehaviourEvent::RelayServer(
            libp2p::relay::Event::CircuitReqAccepted {
                src_peer_id,
                dst_peer_id,
            },
        )) => {
            log::info!("[relay] circuit established: {src_peer_id} -> {dst_peer_id}");
        }
        SwarmEvent::Behaviour(KursalBehaviourEvent::RelayServer(
            libp2p::relay::Event::ReservationTimedOut { src_peer_id },
        )) => {
            log::info!("[relay] reservation timed out for {src_peer_id}");
        }
        SwarmEvent::Behaviour(KursalBehaviourEvent::RelayServer(
            libp2p::relay::Event::CircuitReqDenied {
                src_peer_id,
                dst_peer_id,
                ..
            },
        )) => {
            log::info!("[relay] circuit denied: {src_peer_id} -> {dst_peer_id}");
        }

        SwarmEvent::ConnectionEstablished {
            peer_id, endpoint, ..
        } => {
            let is_relayed_check = endpoint.is_relayed()
                || endpoint
                    .get_remote_address()
                    .to_string()
                    .contains("p2p-circuit");
            let kind = if is_relayed_check {
                ConnectionKind::Relay
            } else {
                ConnectionKind::Direct
            };

            if kind == ConnectionKind::Direct {
                let is_bootstrap = BOOTSTRAP_PEERS.iter().any(|addr| {
                    addr.parse::<Multiaddr>()
                        .ok()
                        .and_then(|ma| {
                            if let Some(Protocol::P2p(id)) = ma.iter().last() {
                                Some(id == peer_id)
                            } else {
                                None
                            }
                        })
                        .unwrap_or(false)
                });

                if is_bootstrap {
                    let circuit_addr = endpoint
                        .get_remote_address()
                        .clone()
                        .with(Protocol::P2pCircuit);
                    let _ = swarm.listen_on(circuit_addr);

                    log::info!("[kad] Bootstrapping Kademlia with relay");
                    let _ = swarm.behaviour_mut().kad.bootstrap();
                }
            }

            let _ = event_tx
                .send(NetworkEvent::ConnectionEstablished { peer_id, via: kind })
                .await;
        }
        SwarmEvent::ConnectionClosed { peer_id, .. } => {
            let _ = event_tx
                .send(NetworkEvent::ConnectionLost { peer_id })
                .await;
        }
        SwarmEvent::Behaviour(KursalBehaviourEvent::RequestResponse(e)) => {
            use request_response::{Event, Message};
            match e {
                Event::Message {
                    peer,
                    message:
                        Message::Request {
                            request, channel, ..
                        },
                    ..
                } => {
                    let _ = swarm
                        .behaviour_mut()
                        .request_response
                        .send_response(channel, vec![]); // INFO: signing is sent from another place
                    let _ = event_tx
                        .send(NetworkEvent::MessageReceived {
                            from: peer,
                            data: request,
                        })
                        .await;
                }
                Event::Message {
                    message: Message::Response { .. },
                    ..
                } => {}
                Event::OutboundFailure { peer, error, .. } => {
                    log::warn!("SendMessage to {peer} failed: {error}");
                    let _ = event_tx
                        .send(NetworkEvent::SendFailed { peer_id: peer })
                        .await;
                }
                _ => {}
            }
        }

        SwarmEvent::NewListenAddr { address, .. } => {
            log::info!("[swarm] listening on {}", address);
            listen_addresses.insert(address);
        }
        SwarmEvent::ExpiredListenAddr { address, .. } => {
            log::info!("[swarm] expired listening on {}", address);
            listen_addresses.remove(&address);
        }

        SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
            log::warn!("[swarm] outgoing connection error peer={peer_id:?} error={error}");
        }
        SwarmEvent::IncomingConnectionError { error, .. } => {
            log::warn!("[swarm] incoming connection error: {error}");
        }

        _ => {}
    }
}

fn handle_swarm_command(
    cmd: SwarmCommand,
    swarm: &mut Swarm<KursalBehaviour>,
    pending_queries: &mut HashMap<libp2p::kad::QueryId, mpsc::Sender<Vec<u8>>>,
    listen_addresses: &mut HashSet<Multiaddr>,
    mdns_enabled: &mut bool,
) {
    match cmd {
        SwarmCommand::Shutdown | SwarmCommand::EnableMdns => {} // handled in the loop itself
        SwarmCommand::Dial(addr) => {
            let _ = swarm.dial(addr);
        }
        SwarmCommand::DisableMdns => {
            *mdns_enabled = false;
            log::info!("mDNS disabled");
        }
        SwarmCommand::PublishDht { key, value } => {
            log::info!("[kad] Putting record into DHT...");
            let record = libp2p::kad::Record {
                key: libp2p::kad::RecordKey::new(&key),
                value,
                publisher: None,
                expires: None,
            };
            match swarm
                .behaviour_mut()
                .kad
                .put_record(record, libp2p::kad::Quorum::One) // TODO: maybe more than one?
            {
                Ok(query_id) => {
                    log::info!("[kad] PutRecord started with query_id: {:?}", query_id);
                }
                Err(err) => {
                    log::error!("[kad] Failed to start PutRecord: {:?}", err);
                }
            }
        }
        SwarmCommand::FetchDht { key, reply_tx } => {
            let query_id = swarm
                .behaviour_mut()
                .kad
                .get_record(libp2p::kad::RecordKey::new(&key));
            pending_queries.insert(query_id, reply_tx);
        }
        SwarmCommand::ContactAdded { contact } => {
            for addr_str in &contact.known_addresses {
                if let Ok(addr) = addr_str.parse::<Multiaddr>()
                    && let Some(Protocol::P2p(peer_id)) = addr.iter().last()
                {
                    swarm.behaviour_mut().kad.add_address(&peer_id, addr);
                }
            }
        }
        SwarmCommand::SendMessage {
            peer_id,
            data,
            addresses,
        } => {
            swarm
                .behaviour_mut()
                .request_response
                .send_request_with_addresses(&peer_id, data, addresses);
        }
        SwarmCommand::GetListenAddresses { reply_tx } => {
            let addrs: Vec<Multiaddr> = listen_addresses.iter().cloned().collect();
            let _ = reply_tx.send(addrs);
        }
    }
}

#[allow(clippy::large_enum_variant)]
pub enum KursalBehaviourEvent {
    Relay(libp2p::relay::client::Event),
    RelayServer(libp2p::relay::Event),
    Dcutr(libp2p::dcutr::Event),
    Kad(libp2p::kad::Event),
    Mdns(libp2p::mdns::Event),
    Identify(libp2p::identify::Event),
    RequestResponse(request_response::Event<Vec<u8>, Vec<u8>>),
}

impl From<libp2p::relay::client::Event> for KursalBehaviourEvent {
    fn from(value: libp2p::relay::client::Event) -> Self {
        Self::Relay(value)
    }
}
impl From<libp2p::relay::Event> for KursalBehaviourEvent {
    fn from(value: libp2p::relay::Event) -> Self {
        Self::RelayServer(value)
    }
}
impl From<libp2p::dcutr::Event> for KursalBehaviourEvent {
    fn from(value: libp2p::dcutr::Event) -> Self {
        Self::Dcutr(value)
    }
}
impl From<libp2p::kad::Event> for KursalBehaviourEvent {
    fn from(value: libp2p::kad::Event) -> Self {
        Self::Kad(value)
    }
}
impl From<libp2p::mdns::Event> for KursalBehaviourEvent {
    fn from(value: libp2p::mdns::Event) -> Self {
        Self::Mdns(value)
    }
}
impl From<libp2p::identify::Event> for KursalBehaviourEvent {
    fn from(value: libp2p::identify::Event) -> Self {
        Self::Identify(value)
    }
}

impl From<request_response::Event<Vec<u8>, Vec<u8>>> for KursalBehaviourEvent {
    fn from(value: request_response::Event<Vec<u8>, Vec<u8>>) -> Self {
        Self::RequestResponse(value)
    }
}

#[derive(Clone, Default)]
pub struct KursalMsgCodec;

#[allow(clippy::cast_possible_truncation)]
impl request_response::Codec for KursalMsgCodec {
    type Protocol = StreamProtocol;
    type Request = Vec<u8>;
    type Response = Vec<u8>;

    fn read_request<'life0, 'life1, 'life2, 'async_trait, T>(
        &'life0 mut self,
        _protocol: &'life1 Self::Protocol,
        io: &'life2 mut T,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = io::Result<Self::Request>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        T: AsyncRead + Unpin + Send,
        T: 'async_trait,
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            let mut len_bytes = [0u8; 4];
            io.read_exact(&mut len_bytes).await?;
            let len = u32::from_be_bytes(len_bytes) as usize;

            if len > MAX_MESSAGE_SIZE {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("message too large: {len} bytes"),
                ));
            }

            let mut buf = vec![0u8; len];
            io.read_exact(&mut buf).await?;
            Ok(buf)
        })
    }

    fn write_request<'life0, 'life1, 'life2, 'async_trait, T>(
        &'life0 mut self,
        _protocol: &'life1 Self::Protocol,
        io: &'life2 mut T,
        req: Self::Request,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = io::Result<()>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        T: AsyncWrite + Unpin + Send,
        T: 'async_trait,
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            if req.len() > MAX_MESSAGE_SIZE {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("message too large to encode: {} bytes", req.len()),
                ));
            }

            let len = (req.len() as u32).to_be_bytes();
            io.write_all(&len).await?;
            io.write_all(&req).await?;
            Ok(())
        })
    }

    fn read_response<'life0, 'life1, 'life2, 'async_trait, T>(
        &'life0 mut self,
        _protocol: &'life1 Self::Protocol,
        _io: &'life2 mut T,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = io::Result<Self::Response>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        T: AsyncRead + Unpin + Send,
        T: 'async_trait,
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move { Ok(vec![]) })
    }

    fn write_response<'life0, 'life1, 'life2, 'async_trait, T>(
        &'life0 mut self,
        _protocol: &'life1 Self::Protocol,
        _io: &'life2 mut T,
        _res: Self::Response,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = io::Result<()>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        T: AsyncWrite + Unpin + Send,
        T: 'async_trait,
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move { Ok(()) })
    }
}

pub async fn get_listen_addrs(cmd_tx: &mpsc::Sender<SwarmCommand>) -> Result<Vec<String>> {
    let (tx, rx) = oneshot::channel();
    cmd_tx
        .send(SwarmCommand::GetListenAddresses { reply_tx: tx })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    let all_addresses = rx
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    let relay_addresses: Vec<String> = all_addresses
        .iter()
        .map(|a| a.to_string())
        .filter(|a| a.contains("/p2p-circuit"))
        .collect();

    Ok(relay_addresses)
}

pub fn str_to_multiaddr(addresses: &[String]) -> Result<Vec<Multiaddr>> {
    addresses
        .iter()
        .map(|el| {
            el.parse::<Multiaddr>()
                .map_err(|err| KursalError::Storage(err.to_string()))
        })
        .collect()
}
