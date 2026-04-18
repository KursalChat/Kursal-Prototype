use crate::{
    KursalError, Result,
    contacts::Contact,
    crypto::{PreKeyBundleData, session_initiate},
    first_contact::{ContactResponse, WireMessage, make_username},
    identity::UserId,
    messaging::enums::MessageId,
    network::{
        NetworkManager,
        swarm::{SwarmCommand, get_listen_addrs, str_to_multiaddr},
    },
    storage::{SharedDatabase, TABLE_LTC_CACHE, get_dilithium_pub, get_timestamp_secs},
};
use libp2p::PeerId;
use libsignal_protocol::{DeviceId, ProtocolAddress};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::str::FromStr;

#[derive(Serialize, Deserialize)]
pub struct LtcPayload {
    pub payload_id: MessageId,
    pub peer_id: String,
    pub pre_key_bundle: Vec<u8>, // no one-time prekey
    pub dilithium_pub_key: Vec<u8>,
    pub relay_addresses: Vec<String>,
    pub created_at: u64,
    pub expires_at: u64,
}

impl LtcPayload {
    pub async fn generate(db: SharedDatabase, network: &NetworkManager) -> Result<Self> {
        let bundle = PreKeyBundleData::build_pre_key_bundle_noprekey(db.clone()).await?;
        let peer_id = network.primary.peer_id.to_base58();
        let dilithium_pub_key = get_dilithium_pub(&*db.0.lock().await)?;

        let created_at = get_timestamp_secs()?;
        let expires_at = created_at + 604800; // 1 week

        let payload_id = MessageId::new();

        db.0.lock()
            .await
            .raw_write(TABLE_LTC_CACHE, "ltc_current_id", &payload_id.0)?;

        let payload = LtcPayload {
            payload_id,
            peer_id,
            pre_key_bundle: bundle.serialize()?,
            dilithium_pub_key,
            relay_addresses: get_listen_addrs(&network.primary.cmd_tx).await?,
            created_at,
            expires_at,
        };

        db.0.lock().await.raw_write(
            TABLE_LTC_CACHE,
            "ltc_current_expiry",
            &expires_at.to_be_bytes(),
        )?;

        Ok(payload)
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|err| KursalError::Storage(err.to_string()))
    }

    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(|err| KursalError::Storage(err.to_string()))
    }

    pub fn is_expired(&self) -> bool {
        let now = get_timestamp_secs().unwrap_or(u64::MAX);
        now > self.expires_at
    }

    pub async fn import_ltc(
        &self,
        db: SharedDatabase,
        network: &NetworkManager,
    ) -> Result<Contact> {
        if self.peer_id == network.primary.peer_id.to_base58() {
            log::debug!("[ltc] Cannot add yourself as a contact");
            return Err(KursalError::Network(
                "Cannot add yourself as a contact".to_string(),
            ));
        }

        let now = get_timestamp_secs()?;

        if self.expires_at < now {
            return Err(KursalError::Identity("LTC expired".to_string()));
        }

        let bundle = PreKeyBundleData::deserialize(&self.pre_key_bundle)?;
        let identity_key_bytes = bundle.identity_key.serialize().to_vec();

        let user_id = UserId(Sha256::digest(&identity_key_bytes).into());
        let remote_address =
            ProtocolAddress::new(hex::encode(user_id.0), DeviceId::new(1u8).unwrap());
        session_initiate(db.clone(), bundle, &remote_address).await?;

        let contact = Contact {
            user_id,
            peer_id: self.peer_id.clone(),
            display_name: make_username(&self.peer_id),
            avatar_bytes: None,
            identity_pub_key: identity_key_bytes,
            dilithium_pub_key: self.dilithium_pub_key.clone(),
            known_addresses: self.relay_addresses.clone(),
            verified: false,
            profile_shared: false,
            blocked: false,
            created_at: now,
        };

        // now build bundle back
        let dilithium_pub_key = get_dilithium_pub(&*db.0.lock().await)?;

        let my_bundle = PreKeyBundleData::build_pre_key_bundle(db.clone()).await?;
        let response = ContactResponse {
            payload_id: self.payload_id,
            pre_key_bundle: my_bundle.serialize()?,
            peer_id: network.primary.peer_id.to_base58(),
            dilithium_pub_key,
            relay_addresses: get_listen_addrs(&network.primary.cmd_tx).await?,
        };

        let wire = WireMessage::ContactResponse(response);
        let response_bytes =
            bincode::serialize(&wire).map_err(|err| KursalError::Storage(err.to_string()))?;

        // send off!
        network
            .primary
            .cmd_tx
            .send(SwarmCommand::SendMessage {
                peer_id: PeerId::from_str(&self.peer_id)
                    .map_err(|err| KursalError::Identity(format!("Invalid peer_id: {err}")))?,
                data: response_bytes,
                addresses: str_to_multiaddr(&contact.known_addresses)?,
            })
            .await
            .map_err(|err| KursalError::Network(err.to_string()))?;

        contact.save(&*db.0.lock().await)?;

        Ok(contact)
    }
}
