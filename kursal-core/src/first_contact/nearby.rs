use crate::{
    KursalError, Result,
    api::AppEvent,
    contacts::Contact,
    crypto::{PreKeyBundleData, session_initiate},
    first_contact::make_username,
    identity::UserId,
    network::swarm::{SwarmCommand, get_listen_addrs},
    storage::{SharedDatabase, get_dilithium_pub, get_timestamp_secs},
};
use libsignal_protocol::{DeviceId, ProtocolAddress};
use rand::{Rng, TryRngCore, distr::Uniform, rngs::OsRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use utoipa::ToSchema;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

pub mod bluetooth;
pub mod mdns;

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug, ToSchema)]
pub enum NearbyOrigin {
    Bluetooth,
    mDNS,
}

const ANIMALS: &str = include_str!("nearby_animals.txt");
const ADJECTIVES: &str = include_str!("nearby_adjectives.txt");

#[derive(Serialize, Deserialize, Clone)]
pub struct NearbyBeacon {
    pub peer_id: String,
    pub session_name: String,
}

impl NearbyBeacon {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|err| KursalError::Storage(err.to_string()))
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(|err| KursalError::Storage(err.to_string()))
    }
}

pub fn generate_session_name() -> Result<String> {
    let mut os_rng = OsRng;
    let mut rng = os_rng.unwrap_mut();

    let animallist: Vec<&str> = ANIMALS.lines().filter(|s| !s.is_empty()).collect();
    let dist_animal =
        Uniform::new(0, animallist.len()).map_err(|err| KursalError::Crypto(err.to_string()))?;

    let adjlist: Vec<&str> = ADJECTIVES.lines().filter(|s| !s.is_empty()).collect();
    let dist_adj =
        Uniform::new(0, adjlist.len()).map_err(|err| KursalError::Crypto(err.to_string()))?;

    let result = format!(
        "{} {}",
        adjlist
            .get(rng.sample(dist_adj))
            .ok_or(KursalError::Crypto(
                "Could not generate a random word".to_string()
            ))?,
        animallist
            .get(rng.sample(dist_animal))
            .ok_or(KursalError::Crypto(
                "Could not generate a random word".to_string()
            ))?,
    );

    Ok(result)
}

#[derive(Serialize, Deserialize)]
pub enum NearbyMessage {
    ConnectRequest {
        from_session_name: String,
    },
    ConnectDecline,
    ConnectAccept {
        bundle: Vec<u8>,
        dilithium_pub: Vec<u8>,
        relay_addresses: Vec<String>,
    },
    BundleReply {
        bundle: Vec<u8>,
        dilithium_pub: Vec<u8>,
        relay_addresses: Vec<String>,
    },
}

impl NearbyMessage {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|err| KursalError::Storage(err.to_string()))
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(|err| KursalError::Storage(err.to_string()))
    }
}

#[allow(async_fn_in_trait)] // trust bro
#[async_trait::async_trait]
pub trait NearbyTransport: Send + Sync {
    async fn start(&self, beacon: NearbyBeacon);
    async fn stop(&self);
    async fn send(&self, peer_id: &str, msg: NearbyMessage) -> Result<()>;
    async fn register_handshake(&self, peer_id: &str) -> mpsc::Receiver<NearbyMessage>;
    async fn unregister_handshake(&self, peer_id: &str);
}

#[derive(Serialize, Deserialize)]
pub enum NearbyPacket {
    Beacon(NearbyBeacon),
    BeaconAck(NearbyBeacon),
    Message(NearbyMessage),
}

impl NearbyPacket {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|err| KursalError::Storage(err.to_string()))
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(|err| KursalError::Storage(err.to_string()))
    }
}

pub enum NearbyRouteResult {
    NotNearby,
    HandledInternally,
    IncomingRequest {
        peer_id: String,
        session_name: String,
    },
}

// Events emitted by BTTransport into dispatch_events
pub enum BtEvent {
    Beacon {
        peer_id: String,
        beacon: NearbyBeacon,
    },
    Message {
        from_peer_id: String,
        msg: NearbyMessage,
    },
}

pub async fn handle_nearby_request(
    from_peer_id: &str,
    transport: &dyn NearbyTransport,
    user_decision: oneshot::Receiver<bool>,
    db: SharedDatabase,
    event_tx: &mpsc::Sender<AppEvent>,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
) -> Result<()> {
    let decision = match tokio::time::timeout(Duration::from_secs(300), user_decision).await {
        Ok(Ok(decision)) => decision,
        _ => {
            // timeout or sender dropped
            let _ = transport
                .send(from_peer_id, NearbyMessage::ConnectDecline)
                .await;
            return Ok(());
        }
    };

    if !decision {
        transport
            .send(from_peer_id, NearbyMessage::ConnectDecline)
            .await?;
        return Ok(());
    }

    let now = get_timestamp_secs()?;

    let mut rx = transport.register_handshake(from_peer_id).await;

    let dilithium_pub_key = get_dilithium_pub(&*db.0.lock().await)?;

    let bundle = PreKeyBundleData::build_pre_key_bundle(db.clone()).await?;
    let bundle_serialized = bundle.serialize()?;

    transport
        .send(
            from_peer_id,
            NearbyMessage::ConnectAccept {
                bundle: bundle_serialized,
                dilithium_pub: dilithium_pub_key,
                relay_addresses: get_listen_addrs(cmd_tx).await?,
            },
        )
        .await?;

    let result = match tokio::time::timeout(Duration::from_secs(30), rx.recv()).await {
        Ok(Some(NearbyMessage::BundleReply {
            bundle,
            dilithium_pub,
            relay_addresses,
        })) => {
            let bundle = PreKeyBundleData::deserialize(&bundle)?;
            let identity_pub_key = bundle.identity_key.serialize().to_vec();

            let user_id = UserId(Sha256::digest(&identity_pub_key).into());
            let address = ProtocolAddress::new(hex::encode(user_id.0), DeviceId::new(1u8).unwrap());

            session_initiate(db.clone(), bundle, &address).await?;

            let contact = Contact {
                user_id,
                peer_id: from_peer_id.to_string(),
                display_name: make_username(from_peer_id),
                avatar_bytes: None,
                identity_pub_key,
                dilithium_pub_key: dilithium_pub,
                known_addresses: relay_addresses,
                verified: false,
                profile_shared: false,
                blocked: false,
                created_at: now,
            };

            {
                let db_lock = db.0.lock().await;
                contact.save(&db_lock)?;
            }

            event_tx
                .send(AppEvent::ContactAdded { contact })
                .await
                .map_err(|err| KursalError::Network(err.to_string()))?;

            Ok(())
        }
        _ => Err(KursalError::Network("No bundle reply received".to_string())),
    };

    transport.unregister_handshake(from_peer_id).await;
    result
}

pub async fn nearby_connect(
    peer_id: &str,
    my_session_name: &str,
    transport: &dyn NearbyTransport,
    db: SharedDatabase,
    event_tx: &mpsc::Sender<AppEvent>,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
) -> Result<()> {
    let now = get_timestamp_secs()?;
    let mut rx = transport.register_handshake(peer_id).await;

    transport
        .send(
            peer_id,
            NearbyMessage::ConnectRequest {
                from_session_name: my_session_name.to_string(),
            },
        )
        .await?;

    let result = match tokio::time::timeout(Duration::from_secs(60), rx.recv()).await {
        Ok(Some(NearbyMessage::ConnectAccept {
            bundle,
            dilithium_pub,
            relay_addresses,
        })) => {
            let bundle = PreKeyBundleData::deserialize(&bundle)?;
            let identity_pub_key = bundle.identity_key.serialize().to_vec();

            let user_id = UserId(Sha256::digest(&identity_pub_key).into());
            let address = ProtocolAddress::new(hex::encode(user_id.0), DeviceId::new(1u8).unwrap());

            session_initiate(db.clone(), bundle, &address).await?;

            let contact = Contact {
                user_id,
                peer_id: peer_id.to_string(),
                display_name: make_username(peer_id),
                avatar_bytes: None,
                identity_pub_key,
                dilithium_pub_key: dilithium_pub,
                known_addresses: relay_addresses,
                verified: false,
                profile_shared: false,
                blocked: false,
                created_at: now,
            };

            // send back
            let our_dilithium = get_dilithium_pub(&*db.0.lock().await)?;

            let our_bundle = PreKeyBundleData::build_pre_key_bundle(db.clone())
                .await?
                .serialize()?;

            transport
                .send(
                    peer_id,
                    NearbyMessage::BundleReply {
                        bundle: our_bundle,
                        dilithium_pub: our_dilithium,
                        relay_addresses: get_listen_addrs(cmd_tx).await?,
                    },
                )
                .await?;

            {
                let db_lock = db.0.lock().await;
                contact.save(&db_lock)?;
            }

            event_tx
                .send(AppEvent::ContactAdded { contact })
                .await
                .map_err(|err| KursalError::Network(err.to_string()))?;

            Ok(())
        }
        Ok(Some(NearbyMessage::ConnectDecline)) => {
            Err(KursalError::Network(
                "Connection declined by peer".to_string(),
            )) // not sure but i think no one emits this for more privacy
        }
        _ => Err(KursalError::Network("No response received".to_string())),
    };

    transport.unregister_handshake(peer_id).await;
    result
}
