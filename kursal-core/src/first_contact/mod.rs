use crate::{
    KursalError, Result,
    api::AppEvent,
    contacts::Contact,
    crypto::{PreKeyBundleData, session_initiate},
    identity::UserId,
    messaging::enums::{KeyRotation, MessageId},
    network::swarm::SwarmCommand,
    storage::{SharedDatabase, TABLE_LTC_CACHE, TABLE_SETTINGS, get_timestamp_secs},
};
use libsignal_protocol::{DeviceId, ProtocolAddress};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;

pub mod ltc;
pub mod nearby;
pub mod otp;

#[derive(Serialize, Deserialize, Clone)]
pub struct ContactResponse {
    pub payload_id: MessageId,
    pub pre_key_bundle: Vec<u8>,
    pub peer_id: String,
    pub dilithium_pub_key: Vec<u8>,
    pub relay_addresses: Vec<String>,
}

enum HandshakeKind {
    Otp,
    Ltc,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FileTransferMessage {
    pub transfer_id: [u8; 16],
    pub index: u32,
    pub data: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub enum WireMessage {
    Encrypted(Vec<u8>),                   // for normal encrypted libsignal
    ContactResponse(ContactResponse),     // for OTP / LTC handshake
    KeyRotationAnnouncement(KeyRotation), // for peer id rotation
    FileTransfer(FileTransferMessage),
}

pub fn make_username(peer_id: &str) -> String {
    let hash = Sha256::digest(peer_id.as_bytes());
    let suffix = hex::encode(&hash[..3]);

    format!("Unknown #{suffix}")
}

pub async fn handle_fc_response(
    response: ContactResponse,
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    event_tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    let now = get_timestamp_secs()?;

    let handshake = {
        let db_lock = db.0.lock().await;

        let otp_match = match db_lock.raw_read(TABLE_SETTINGS, "otp_pending_id")? {
            Some(id) if id.as_slice() == response.payload_id.0.as_slice() => {
                let published_at: u64 = db_lock
                    .raw_read(TABLE_SETTINGS, "otp_published_at")?
                    .and_then(|b| b.try_into().ok().map(u64::from_be_bytes))
                    .unwrap_or(0);
                now.saturating_sub(published_at) <= 600
            }
            _ => false,
        };

        let ltc_match = match db_lock.raw_read(TABLE_LTC_CACHE, "ltc_current_id")? {
            Some(id) if id.as_slice() == response.payload_id.0.as_slice() => db_lock
                .raw_read(TABLE_LTC_CACHE, "ltc_current_expiry")?
                .and_then(|b| b.try_into().ok().map(u64::from_be_bytes))
                .map(|expiry: u64| now <= expiry)
                .unwrap_or(false),
            _ => false,
        };

        if otp_match {
            Some(HandshakeKind::Otp)
        } else if ltc_match {
            Some(HandshakeKind::Ltc)
        } else {
            None
        }
    };

    let Some(handshake) = handshake else {
        log::warn!("Rejected incoming ContactResponse: payload_id matches no pending handshake");
        return Ok(());
    };

    let bundle = PreKeyBundleData::deserialize(&response.pre_key_bundle)?;
    let identity_key_bytes = bundle.identity_key.serialize().to_vec();

    let user_id = UserId(Sha256::digest(&identity_key_bytes).into());
    let bob_address = ProtocolAddress::new(hex::encode(user_id.0), DeviceId::new(1u8).unwrap());
    session_initiate(db.clone(), bundle, &bob_address).await?;

    let contact = Contact {
        user_id,
        peer_id: response.peer_id.clone(),
        display_name: make_username(&response.peer_id),
        avatar_bytes: None,
        identity_pub_key: identity_key_bytes,
        dilithium_pub_key: response.dilithium_pub_key,
        known_addresses: response.relay_addresses,
        verified: false,
        profile_shared: false,
        blocked: false,
        created_at: now,
    };

    {
        let db_lock = db.0.lock().await;
        contact.save(&db_lock)?;
        if matches!(handshake, HandshakeKind::Otp) {
            db_lock.raw_delete(TABLE_SETTINGS, "otp_pending_id")?;
            db_lock.raw_delete(TABLE_SETTINGS, "otp_published_at")?;
        }
    }

    cmd_tx
        .send(SwarmCommand::ContactAdded {
            contact: contact.clone(),
        })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    event_tx
        .send(AppEvent::ContactAdded { contact })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    Ok(())
}
