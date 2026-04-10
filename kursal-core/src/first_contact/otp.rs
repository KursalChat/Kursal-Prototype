use crate::{
    KursalError, Result,
    api::AppEvent,
    contacts::Contact,
    crypto::{
        PreKeyBundleData, session_initiate,
        stream::{stream_decrypt, stream_encrypt},
    },
    first_contact::{ContactResponse, WireMessage, make_username},
    identity::UserId,
    network::{
        NetworkManager,
        dht::DHTRecord,
        swarm::{SwarmCommand, get_listen_addrs, str_to_multiaddr},
    },
    storage::{
        SharedDatabase, TABLE_LTC_CACHE, TABLE_SETTINGS, get_dilithium_pub, get_timestamp_secs,
    },
};
use argon2::{Argon2, ParamsBuilder};
use libp2p::PeerId;
use libsignal_protocol::{DeviceId, ProtocolAddress};
use rand::{Rng, TryRngCore, distr::Uniform, rngs::OsRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{str::FromStr, time::Duration};
use tokio::sync::mpsc;

const SALT: &[u8; 16] = b"kursal-otp-salt1";
const WORDS: &str = include_str!("otp_wordlist.txt");

pub fn generate_otp() -> Result<String> {
    let wordlist: Vec<&str> = WORDS.lines().filter(|s| !s.is_empty()).collect();

    let dist =
        Uniform::new(0, wordlist.len()).map_err(|err| KursalError::Crypto(err.to_string()))?;
    let mut os_rng = OsRng;
    let mut rng = os_rng.unwrap_mut();

    // 8 random words
    let result: Result<Vec<String>> = (0..8)
        .map(|_| {
            wordlist
                .get(rng.sample(dist))
                .ok_or(KursalError::Crypto(
                    "Could not generate a random word".to_string(),
                ))
                .map(|w| w.to_string())
        })
        .collect();

    Ok(result?.join(" "))
}

pub fn hash_otp(otp: &str) -> Result<[u8; 32]> {
    let argon2 = Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        ParamsBuilder::new()
            .m_cost(256 * 1024)
            .t_cost(2)
            .p_cost(1)
            .output_len(32)
            .build()
            .map_err(|err| KursalError::Crypto(err.to_string()))?,
    );

    let mut output = [0u8; 32];
    argon2
        .hash_password_into(otp.as_bytes(), SALT, &mut output)
        .map_err(|err| KursalError::Crypto(err.to_string()))?;

    Ok(output)
}

pub fn otp_to_keys(otp: &str) -> Result<([u8; 32], [u8; 32])> {
    let hash = hash_otp(otp)?;
    let dht_key = Sha256::digest(hash).into();

    Ok((hash, dht_key))
}

#[derive(Serialize, Deserialize)]
pub struct OtpPayload {
    pub pre_key_bundle: Vec<u8>,
    pub peer_id: String,
    pub dilithium_pub_key: Vec<u8>,
    pub relay_addresses: Vec<String>,
}

impl OtpPayload {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|err| KursalError::Storage(err.to_string()))
    }

    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(|err| KursalError::Storage(err.to_string()))
    }
}

pub async fn build_otp_payload(
    db: SharedDatabase,
    network: &NetworkManager,
    enc_key: &[u8; 32],
) -> Result<Vec<u8>> {
    let bundle = PreKeyBundleData::build_pre_key_bundle(db.clone())
        .await?
        .serialize()?;
    let peer_id = network.primary.peer_id.to_base58();
    let dilithium_pub_key = get_dilithium_pub(&*db.0.lock().await)?;

    let payload = OtpPayload {
        pre_key_bundle: bundle,
        peer_id,
        dilithium_pub_key,
        relay_addresses: get_listen_addrs(&network.primary.cmd_tx).await?,
    };

    stream_encrypt(enc_key, &payload.serialize()?)
}

// TODO: publish from new identity + decoys
pub async fn publish_otp(otp: &str, db: SharedDatabase, network: &NetworkManager) -> Result<()> {
    let timestamp = get_timestamp_secs()?;

    let (enc_key, dht_key) = otp_to_keys(otp)?;
    let payload = build_otp_payload(db.clone(), network, &enc_key).await?;

    let secret_key_bytes =
        db.0.lock()
            .await
            .raw_read(TABLE_SETTINGS, "dilithium_secret")?
            .ok_or(KursalError::Crypto("No signing key stored".to_string()))?;

    let dht_record = DHTRecord::new(dht_key.to_vec(), secret_key_bytes, payload, timestamp).await?;

    network
        .primary
        .cmd_tx
        .send(SwarmCommand::PublishDht {
            key: dht_key.to_vec(),
            value: dht_record.serialize()?,
        })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    {
        let db_lock = db.0.lock().await;
        db_lock.raw_write(TABLE_SETTINGS, "otp_published_at", &timestamp.to_be_bytes())?;
        db_lock.raw_write(TABLE_SETTINGS, "otp_pending", &[1u8])?;
    }

    Ok(())
}

pub async fn fetch_otp(otp: &str, db: SharedDatabase, network: &NetworkManager) -> Result<Contact> {
    let local_peer_id = network.primary.peer_id.to_base58();
    let timestamp = get_timestamp_secs()?;
    let (enc_key, dht_key) = otp_to_keys(otp)?;

    let (reply_tx, mut reply_rx) = mpsc::channel(16);

    network
        .primary
        .cmd_tx
        .send(SwarmCommand::FetchDht {
            key: dht_key.to_vec(),
            reply_tx,
        })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    let timeout = Duration::from_secs(30);

    let (payload, bundle) = tokio::time::timeout(timeout, async {
        while let Some(bytes) = reply_rx.recv().await {
            let dht_record = match DHTRecord::is_valid(&dht_key, &bytes) {
                Ok(record) => record,
                Err(err) => {
                    log::debug!("[otp] Ignoring invalid DHT record: {err}");
                    continue;
                }
            };

            let decrypted = match stream_decrypt(&enc_key, &dht_record.value) {
                Ok(decrypted) => decrypted,
                Err(err) => {
                    log::debug!("[otp] DHT record decrypted failed: {err}");
                    continue;
                }
            };

            let record = match OtpPayload::deserialize(&decrypted) {
                Ok(record) => record,
                Err(err) => {
                    log::debug!("[otp] OTP payload deserialize failed: {err}");
                    continue;
                }
            };

            if record.peer_id == local_peer_id {
                log::debug!("[otp] Cannot add yourself as a contact");
                continue;
            }

            if let Ok(bundle) = PreKeyBundleData::deserialize(&record.pre_key_bundle) {
                // decoded. yay!
                return Some((record, bundle));
            }
        }
        None
    })
    .await
    .map_err(|err| KursalError::Network(err.to_string()))?
    .ok_or(KursalError::Network("Record not found".to_string()))?;

    let identity_pub_key = bundle.identity_key.serialize().to_vec();
    let user_id: [u8; 32] = Sha256::digest(&identity_pub_key).into();

    let remote_address = ProtocolAddress::new(hex::encode(user_id), DeviceId::new(1u8).unwrap());
    session_initiate(db.clone(), bundle, &remote_address).await?;

    let contact = Contact {
        user_id: UserId(user_id),
        peer_id: payload.peer_id.clone(),
        display_name: make_username(&payload.peer_id),
        avatar_bytes: None,
        identity_pub_key,
        dilithium_pub_key: payload.dilithium_pub_key,
        known_addresses: payload.relay_addresses,
        verified: false,
        profile_shared: false,
        blocked: false,
        created_at: timestamp,
    };

    // now build bundle back
    let dilithium_pub_key = get_dilithium_pub(&*db.0.lock().await)?;

    let my_bundle = PreKeyBundleData::build_pre_key_bundle(db.clone()).await?;
    let response = ContactResponse {
        pre_key_bundle: my_bundle.serialize()?,
        peer_id: network.primary.peer_id.to_base58(),
        dilithium_pub_key,
        relay_addresses: get_listen_addrs(&network.primary.cmd_tx).await?,
    };

    let wire = WireMessage::ContactResponse(response);
    let response_bytes =
        bincode::serialize(&wire).map_err(|err| KursalError::Storage(err.to_string()))?;

    // and send
    network
        .primary
        .cmd_tx
        .send(SwarmCommand::SendMessage {
            peer_id: PeerId::from_str(&payload.peer_id)
                .map_err(|err| KursalError::Network(err.to_string()))?,
            data: response_bytes,
            addresses: str_to_multiaddr(&contact.known_addresses)?,
        })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    contact.save(&*db.0.lock().await)?;
    Ok(contact)
}

pub async fn handle_otp_response(
    response: ContactResponse,
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    event_tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    let now = get_timestamp_secs()?;

    // Check if OTP handshake is pending
    let (otp_pending, otp_valid) = {
        let db_lock = db.0.lock().await;
        let pending = db_lock
            .raw_read(TABLE_SETTINGS, "otp_pending")?
            .map(|b| b.first() == Some(&1))
            .unwrap_or(false);

        let published_at: u64 = db_lock
            .raw_read(TABLE_SETTINGS, "otp_published_at")?
            .and_then(|b| b.try_into().ok().map(u64::from_be_bytes))
            .unwrap_or(0);

        (pending, now.saturating_sub(published_at) <= 600)
    };

    // Check if LTC handshake is pending
    let ltc_valid = {
        let db_lock = db.0.lock().await;
        if let Ok(Some(bytes)) = db_lock.raw_read(TABLE_LTC_CACHE, "ltc_current_expiry") {
            if bytes.len() == 8 {
                let expiry = u64::from_be_bytes(bytes.try_into().unwrap());
                now <= expiry
            } else {
                false
            }
        } else {
            false
        }
    };

    if !(ltc_valid || otp_pending && otp_valid) {
        log::warn!("Rejected incoming ContactResponse: neither OTP nor LTC is pending or valid");
        return Ok(()); // silent ignore
    }

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
        if otp_pending {
            // FIXME: if LTC is used and OTP is pending it will still clear OTP
            db_lock.raw_write(TABLE_SETTINGS, "otp_pending", &[0u8])?;
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
