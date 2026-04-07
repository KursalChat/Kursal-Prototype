use crate::{
    config::RelayConfig,
    health::{HealthState, start_health_server},
    identity::load_or_generate,
    limiter::ConnectionLimiter,
};
use kursal_core::{
    KursalError, Result,
    network::{BOOTSTRAP_PEERS, kademlia::KursalKadStore},
};
use libp2p::{
    Multiaddr, PeerId, StreamProtocol, Swarm, SwarmBuilder,
    futures::StreamExt,
    multiaddr::Protocol,
    swarm::{NetworkBehaviour, SwarmEvent},
};
use std::{collections::HashSet, convert::Infallible, path::Path, sync::Arc, time::Instant};
use tokio::sync::RwLock;

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "KursalBehaviourEvent")]
pub struct KursalBehaviour {
    pub relay: libp2p::relay::Behaviour,
    pub kad: libp2p::kad::Behaviour<KursalKadStore>,
    pub identify: libp2p::identify::Behaviour,
    pub limiter: ConnectionLimiter,
}

pub struct RelayHandle {
    pub peer_id: PeerId,
}

pub async fn spawn_relay_swarm(config: &RelayConfig, config_path: &Path) -> Result<()> {
    let keypair = load_or_generate(config_path)?;
    let peer_id = keypair.public().to_peer_id();
    log::info!("[relay] peer id: {peer_id}");

    let mut swarm = SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_tcp(
            Default::default(),
            libp2p::noise::Config::new,
            libp2p::yamux::Config::default,
        )
        .map_err(|err| KursalError::Network(err.to_string()))?
        .with_quic()
        .with_dns()
        .map_err(|err| KursalError::Network(err.to_string()))?
        .with_behaviour(
            |key| -> std::result::Result<_, Box<dyn std::error::Error + Send + Sync>> {
                let local_peer_id = key.public().to_peer_id();

                let relay = libp2p::relay::Behaviour::new(local_peer_id, Default::default());

                let mut kad = libp2p::kad::Behaviour::with_config(
                    local_peer_id,
                    KursalKadStore::new(local_peer_id),
                    libp2p::kad::Config::new(StreamProtocol::new("/kursal/kad/1.0.0")),
                );
                kad.set_mode(Some(libp2p::kad::Mode::Server));

                let identify = libp2p::identify::Behaviour::new(libp2p::identify::Config::new(
                    "/kursal/v1.0.0".to_string(),
                    key.public(),
                ));

                let limiter =
                    ConnectionLimiter::new(config.max_connections, config.max_connections_per_ip);

                Ok(KursalBehaviour {
                    identify,
                    kad,
                    relay,
                    limiter,
                })
            },
        )
        .map_err(|err| KursalError::Network(err.to_string()))?
        .build();

    let tcp_addr =
        Multiaddr::from(config.listen_addr.ip()).with(Protocol::Tcp(config.listen_addr.port()));
    let quic_addr = Multiaddr::from(config.listen_addr.ip())
        .with(Protocol::Udp(config.listen_addr.port()))
        .with(Protocol::QuicV1);

    swarm
        .listen_on(tcp_addr)
        .map_err(|err| KursalError::Network(err.to_string()))?;
    swarm
        .listen_on(quic_addr)
        .map_err(|err| KursalError::Network(err.to_string()))?;

    let tcp_addr =
        Multiaddr::from(config.announce_addr.ip()).with(Protocol::Tcp(config.announce_addr.port()));
    let quic_addr = Multiaddr::from(config.announce_addr.ip())
        .with(Protocol::Udp(config.listen_addr.port()))
        .with(Protocol::QuicV1);

    swarm.add_external_address(tcp_addr);
    swarm.add_external_address(quic_addr);

    for addr_str in BOOTSTRAP_PEERS {
        if let Ok(multiaddr) = addr_str.parse::<Multiaddr>()
            && let Some(Protocol::P2p(peer_id)) = multiaddr.iter().last()
        {
            swarm
                .behaviour_mut()
                .kad
                .add_address(&peer_id, multiaddr.clone());
            let _ = swarm.dial(multiaddr);
        }
    }

    for multiaddr in &config.bootstrap_peers {
        if let Some(Protocol::P2p(peer_id)) = multiaddr.iter().last() {
            swarm
                .behaviour_mut()
                .kad
                .add_address(&peer_id, multiaddr.clone());
            let _ = swarm.dial(multiaddr.clone());
        }
    }

    log::info!("[swarm] event loop started");

    let health_state = Arc::new(RwLock::new(HealthState {
        peer_id: peer_id.to_base58(),
        start_time: Instant::now(),
        connections: 0,
    }));

    tokio::spawn(start_health_server(
        health_state.clone(),
        config.health.listen_addr.parse().unwrap(),
    ));

    let mut listen_addresses: HashSet<Multiaddr> = HashSet::new();
    let mut connection_count = 0usize;

    loop {
        tokio::select! {
            event = swarm.select_next_some() => handle_swarm_event(
                event,
                &mut swarm,
                &mut listen_addresses,
                &mut connection_count,
                health_state.clone(),
            )
            .await,
            _ = tokio::signal::ctrl_c() => {
                log::info!("[relay] shutting down");
                break;
            }
        }
    }

    Ok(())
}

async fn handle_swarm_event(
    event: SwarmEvent<KursalBehaviourEvent>,
    swarm: &mut Swarm<KursalBehaviour>,
    listen_addresses: &mut HashSet<Multiaddr>,
    connection_count: &mut usize,
    health_state: Arc<RwLock<HealthState>>,
) {
    match event {
        SwarmEvent::Behaviour(KursalBehaviourEvent::Identify(
            libp2p::identify::Event::Received { peer_id, info, .. },
        )) => {
            for addr in info.listen_addrs.clone() {
                swarm.behaviour_mut().kad.add_address(&peer_id, addr);
            }
        }

        SwarmEvent::Behaviour(KursalBehaviourEvent::Relay(
            libp2p::relay::Event::ReservationReqAccepted { src_peer_id, .. },
        )) => {
            log::info!("[relay] reservation accepted from {src_peer_id}");
        }
        SwarmEvent::Behaviour(KursalBehaviourEvent::Relay(
            libp2p::relay::Event::CircuitReqAccepted {
                src_peer_id,
                dst_peer_id,
            },
        )) => {
            log::info!("[relay] circuit established: {src_peer_id} -> {dst_peer_id}");
        }
        SwarmEvent::Behaviour(KursalBehaviourEvent::Relay(
            libp2p::relay::Event::ReservationTimedOut { src_peer_id },
        )) => {
            log::info!("[relay] reservation timed out for {src_peer_id}");
        }
        SwarmEvent::Behaviour(KursalBehaviourEvent::Relay(
            libp2p::relay::Event::CircuitReqDenied {
                src_peer_id,
                dst_peer_id,
                ..
            },
        )) => {
            log::info!("[relay] circuit denied: {src_peer_id} -> {dst_peer_id}");
        }

        SwarmEvent::Behaviour(KursalBehaviourEvent::Kad(libp2p::kad::Event::RoutingUpdated {
            peer,
            ..
        })) => {
            log::debug!("[kad] routing table updated: {peer}");
        }
        SwarmEvent::Behaviour(KursalBehaviourEvent::Kad(libp2p::kad::Event::InboundRequest {
            request,
        })) => {
            log::debug!("[kad] inbound request: {request:?}");
        }
        SwarmEvent::Behaviour(KursalBehaviourEvent::Kad(_)) => {}

        SwarmEvent::ConnectionEstablished {
            peer_id, endpoint, ..
        } => {
            *connection_count += 1;

            let kind_str = if endpoint.is_relayed() {
                "relay".to_string()
            } else {
                "direct".to_string()
            };

            log::info!("[swarm] connected: {peer_id} via {kind_str}  (total {connection_count})");

            let mut health = health_state.write().await;
            health.connections = *connection_count;
        }
        SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
            *connection_count = connection_count.saturating_sub(1);

            match cause {
                Some(err) => log::info!("[swarm] disconnected: {peer_id} - {err}"),
                None => log::info!("[swarm] disconnected: {peer_id}"),
            }

            let mut health = health_state.write().await;
            health.connections = *connection_count;
        }

        SwarmEvent::NewListenAddr { address, .. } => {
            log::info!("[swarm] listening on {}", address);
            listen_addresses.insert(address);
        }
        SwarmEvent::ExpiredListenAddr { address, .. } => {
            log::info!("[swarm] expired listening on {}", address);
            listen_addresses.remove(&address);
        }
        _ => {}
    }
}

#[allow(clippy::large_enum_variant)]
pub enum KursalBehaviourEvent {
    Relay(libp2p::relay::Event),
    Kad(libp2p::kad::Event),
    Identify(libp2p::identify::Event),
    Limiter(Infallible),
}

impl From<libp2p::relay::Event> for KursalBehaviourEvent {
    fn from(value: libp2p::relay::Event) -> Self {
        Self::Relay(value)
    }
}
impl From<libp2p::kad::Event> for KursalBehaviourEvent {
    fn from(value: libp2p::kad::Event) -> Self {
        Self::Kad(value)
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
