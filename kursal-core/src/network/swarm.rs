use crate::{
    KursalError, Result,
    api::handle_incoming::handle_incoming_stream,
    contacts::Contact,
    identity::TransportIdentity,
    network::{
        BOOTSTRAP_PEERS,
        kademlia::{KAD_MAX_PAYLOAD, KursalKadStore},
        limiter::ConnectionLimiter,
    },
    storage::RelayConfig,
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
use std::time::Duration;
use std::{
    collections::{HashMap, HashSet},
    net::IpAddr,
};
use std::{convert::Infallible, io};
use tokio::sync::{mpsc, oneshot};

pub const STREAM_PROTOCOL: StreamProtocol = StreamProtocol::new("/kursal/transfer/1.0.0");
pub const MAX_MESSAGE_SIZE: usize = 512 * 1024; // 512 KB, should LARGE be enough
pub const FILE_CHUNK_SIZE: usize = 64 * 1024;

pub enum SwarmCommand {
    Dial(Multiaddr),
    SendMessage {
        peer_id: PeerId,
        data: Vec<u8>,
        addresses: Vec<Multiaddr>,
    },
    OpenStream {
        peer_id: PeerId,
        addresses: Vec<Multiaddr>,
        reply: oneshot::Sender<Option<mpsc::Sender<Vec<u8>>>>,
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
    EnableNearby,
    DisableNearby,
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
    pub mdns: Toggle<libp2p::mdns::tokio::Behaviour>,
    pub identify: libp2p::identify::Behaviour,
    pub request_response: request_response::Behaviour<KursalMsgCodec>,
    pub streaming: libp2p_stream::Behaviour,
    pub limiter: ConnectionLimiter,
}

pub struct SwarmHandle {
    pub peer_id: PeerId,
    pub cmd_tx: mpsc::Sender<SwarmCommand>,
    pub relay_config: RelayConfig,
    pub mdns_enabled: bool,
    pub port: u16,
}

impl SwarmHandle {
    pub async fn spawn(
        identity: TransportIdentity,
        event_tx: mpsc::Sender<NetworkEvent>,
        relay_config: RelayConfig,
        mdns_enabled: bool,
        port: u16,
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

                let mdns = if mdns_enabled {
                    Toggle::from(Some(libp2p::mdns::tokio::Behaviour::new(mdns::Config {
                        query_interval: Duration::from_secs(60),
                        ttl: Duration::from_secs(2 * 60),
                        enable_ipv6: false,
                    }, local_peer_id)?))
                } else {
                    log::info!("mDNS disabled");
                    Toggle::from(None)
                };

                let identify = libp2p::identify::Behaviour::new(libp2p::identify::Config::new(
                    "/kursal/v1.0.0".to_string(),
                    key.public(),
                ));

                let request_response = request_response::Behaviour::new(
                    [(StreamProtocol::new("/kursal/msg/1.0.0"), ProtocolSupport::Full)],
                    request_response::Config::default(),
                );

                let relay_server = if relay_config.enabled {
                    Toggle::from(Some(libp2p::relay::Behaviour::new(local_peer_id, libp2p::relay::Config {
                        max_circuits: 1024,
                        max_circuits_per_peer: 32,
                        max_reservations: 1024,
                        ..Default::default()
                    })))
                } else {
                    Toggle::from(None)
                };

                let streaming = libp2p_stream::Behaviour::new();

                let limiter = ConnectionLimiter::new(relay_config.max_connections, relay_config.max_connections_per_ip);

                Ok(KursalBehaviour {
                    relay,
                    relay_server,
                    dcutr,
                    kad,
                    mdns,
                    identify,
                    request_response,
                    streaming,
                    limiter
                })
            })
            .map_err(|err| KursalError::Network(format!("swarm behaviour error: {err}")))?
            .build();

        swarm
            .listen_on(format!("/ip4/0.0.0.0/tcp/{port}").parse().unwrap())
            .map_err(|err| KursalError::Network(format!("swarm listen error: {err}")))?;
        swarm
            .listen_on(format!("/ip4/0.0.0.0/udp/{port}/quic-v1").parse().unwrap())
            .map_err(|err| KursalError::Network(format!("swarm listen error: {err}")))?;

        let (cmd_tx, mut cmd_rx) = mpsc::channel::<SwarmCommand>(32);
        let peer_id = identity.peer_id;

        let mut incoming_control = swarm.behaviour().streaming.new_control();
        let incoming_event_tx = event_tx.clone();
        tokio::spawn(async move {
            let mut incoming = incoming_control.accept(STREAM_PROTOCOL).unwrap();
            while let Some((peer_id, stream)) = incoming.next().await {
                tokio::spawn(handle_incoming_stream(
                    peer_id,
                    stream,
                    incoming_event_tx.clone(),
                ));
            }
        });

        tokio::spawn(async move {
            log::info!("[swarm] event loop started");
            let mut pending_queries: HashMap<libp2p::kad::QueryId, mpsc::Sender<Vec<u8>>> =
                HashMap::new();
            let mut listen_addresses: HashSet<Multiaddr> = HashSet::new();
            let mut nearby_enabled = false;
            let mut mdns_peers: HashMap<PeerId, Multiaddr> = HashMap::new();

            let mut stream_control = swarm.behaviour().streaming.new_control();
            let mut peer_streams: HashMap<PeerId, mpsc::Sender<Vec<u8>>> = HashMap::new();

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
                    event = swarm.select_next_some() => handle_swarm_event(event, &event_tx, &mut pending_queries, &mut listen_addresses, &mut swarm, nearby_enabled, &mut mdns_peers).await,
                    cmd = cmd_rx.recv() => {
                        match cmd {
                            Some(SwarmCommand::Shutdown) => break,
                            Some(SwarmCommand::EnableNearby) => {
                                nearby_enabled = true;
                                for (peer_id, addr) in mdns_peers.iter() {
                                    let _ = event_tx.send(NetworkEvent::PeerDiscovered { peer_id: *peer_id, addresses: vec![addr.clone()] }).await;
                                }

                                log::info!("Nearby enabled ({} known mdns peers)", mdns_peers.len());
                            },
                            Some(cmd) => handle_swarm_command(cmd, &mut swarm, &mut pending_queries, &mut listen_addresses, &mut nearby_enabled, &mut stream_control, &mut peer_streams).await,
                            None => break
                        }
                    }
                }
            }
        });

        Ok(SwarmHandle {
            peer_id,
            cmd_tx,
            relay_config,
            mdns_enabled,
            port,
        })
    }
}

async fn handle_swarm_event(
    event: SwarmEvent<KursalBehaviourEvent>,
    event_tx: &mpsc::Sender<NetworkEvent>,
    pending_queries: &mut HashMap<libp2p::kad::QueryId, mpsc::Sender<Vec<u8>>>,
    listen_addresses: &mut HashSet<Multiaddr>,
    swarm: &mut Swarm<KursalBehaviour>,
    nearby_enabled: bool,
    mdns_peers: &mut HashMap<PeerId, Multiaddr>,
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
                if !is_routable_multiaddr(&addr) {
                    log::trace!("[identify] skip unroutable {addr} from {peer_id}");
                    continue;
                }

                swarm.behaviour_mut().kad.add_address(&peer_id, addr);
            }
        }
        SwarmEvent::Behaviour(KursalBehaviourEvent::Mdns(libp2p::mdns::Event::Discovered(
            peers,
        ))) => {
            log::info!(
                "[mDNS] raw Discovered event, nearby_enabled={}, peers={}",
                nearby_enabled,
                peers.len()
            );

            for (peer_id, addr) in peers {
                if !is_routable_multiaddr(&addr) {
                    log::trace!("[mDNS] skip unroutable {addr} from {peer_id}");
                    continue;
                }

                log::info!("[mDNS] discovered peer {} at {}", peer_id, addr);

                mdns_peers.insert(peer_id, addr.clone());

                if nearby_enabled {
                    let _ = event_tx
                        .send(NetworkEvent::PeerDiscovered {
                            peer_id,
                            addresses: vec![addr],
                        })
                        .await;
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
            log::info!("[swarm] outgoing connection error peer={peer_id:?} error={error}");
        }
        SwarmEvent::IncomingConnectionError { error, .. } => {
            log::info!("[swarm] incoming connection error: {error}");
        }

        _ => {}
    }
}

async fn handle_swarm_command(
    cmd: SwarmCommand,
    swarm: &mut Swarm<KursalBehaviour>,
    pending_queries: &mut HashMap<libp2p::kad::QueryId, mpsc::Sender<Vec<u8>>>,
    listen_addresses: &mut HashSet<Multiaddr>,
    nearby_enabled: &mut bool,
    stream_control: &mut libp2p_stream::Control,
    peer_streams: &mut HashMap<PeerId, mpsc::Sender<Vec<u8>>>,
) {
    match cmd {
        SwarmCommand::Shutdown | SwarmCommand::EnableNearby => {} // handled in the loop itself
        SwarmCommand::Dial(addr) => {
            let _ = swarm.dial(addr);
        }
        SwarmCommand::DisableNearby => {
            *nearby_enabled = false;
            log::info!("Nearby disabled");
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
        SwarmCommand::OpenStream {
            peer_id,
            addresses,
            reply,
        } => {
            for addr in addresses {
                let _ = swarm.dial(addr);
            }

            let sender = if let Some(tx) = peer_streams.get(&peer_id) {
                if !tx.is_closed() {
                    Some(tx.clone())
                } else {
                    peer_streams.remove(&peer_id);
                    open_peer_stream(stream_control, peer_id, peer_streams).await
                }
            } else {
                open_peer_stream(stream_control, peer_id, peer_streams).await
            };

            let _ = reply.send(sender);
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
    Limiter(Infallible),
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
impl From<Infallible> for KursalBehaviourEvent {
    fn from(value: Infallible) -> Self {
        Self::Limiter(value)
    }
}

impl From<request_response::Event<Vec<u8>, Vec<u8>>> for KursalBehaviourEvent {
    fn from(value: request_response::Event<Vec<u8>, Vec<u8>>) -> Self {
        Self::RequestResponse(value)
    }
}
impl From<()> for KursalBehaviourEvent {
    fn from(_: ()) -> Self {
        unreachable!()
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

pub async fn open_peer_stream(
    control: &mut libp2p_stream::Control,
    peer_id: PeerId,
    peer_streams: &mut HashMap<PeerId, mpsc::Sender<Vec<u8>>>,
) -> Option<mpsc::Sender<Vec<u8>>> {
    let mut control = control.clone();

    let mut stream = match control.open_stream(peer_id, STREAM_PROTOCOL).await {
        Ok(s) => s,
        Err(e) => {
            log::warn!("failed to open stream to {peer_id}: {e}");
            return None;
        }
    };

    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(32);

    tokio::spawn(async move {
        while let Some(data) = rx.recv().await {
            let Ok(len) =
                u32::try_from(data.len()).map_err(|err| KursalError::Storage(err.to_string()))
            else {
                break;
            };

            if stream.write_all(&len.to_be_bytes()).await.is_err() {
                break;
            }
            if stream.write_all(&data).await.is_err() {
                break;
            }
        }

        stream.close().await.ok();
    });

    peer_streams.insert(peer_id, tx.clone());
    Some(tx)
}

pub fn is_routable_multiaddr(addr: &Multiaddr) -> bool {
    for proto in addr.iter() {
        match proto {
            Protocol::Ip4(ip) => {
                if ip.is_loopback() || ip.is_link_local() || ip.is_unspecified() {
                    return false;
                }
            }
            Protocol::Ip6(ip) => {
                let is_link_local = (ip.segments()[0] & 0xffc0) == 0xfe80;
                if ip.is_loopback() || ip.is_unspecified() || is_link_local {
                    return false;
                }
                let _ = IpAddr::V6(ip);
            }
            _ => {}
        }
    }
    true
}
