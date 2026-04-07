use crate::{
    KursalError, Result,
    network::{
        NetworkManager,
        dht::DHTRecord,
        swarm::{SwarmCommand, get_listen_addrs},
    },
    storage::{Database, TABLE_IDENTITY_KEYS, TABLE_SETTINGS, get_timestamp_secs},
};
use libsignal_protocol::IdentityKeyPair;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Serialize, Deserialize)]
pub struct RendezvousRecord {
    pub peer_id: String,
    pub relay_addresses: Vec<String>,
}

impl RendezvousRecord {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|err| KursalError::Storage(err.to_string()))
    }

    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(|err| KursalError::Storage(err.to_string()))
    }
}

#[allow(clippy::cast_possible_truncation)]
pub async fn publish_rendezvous(
    db: &Database,
    peer_id: String,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
) -> Result<()> {
    let relay_addresses = get_listen_addrs(cmd_tx).await?;
    let timestamp = get_timestamp_secs()?;

    let secret_key_bytes = db
        .raw_read(TABLE_SETTINGS, "dilithium_secret")?
        .ok_or(KursalError::Crypto("No signing key stored".to_string()))?;

    let record = RendezvousRecord {
        peer_id,
        relay_addresses,
    };

    let identity_bytes = db
        .raw_read(TABLE_IDENTITY_KEYS, "local_identity")?
        .ok_or_else(|| KursalError::Identity("Identity not found".to_string()))?;

    let identity_keypair = IdentityKeyPair::try_from(identity_bytes.as_slice())
        .map_err(|e| KursalError::Identity(e.to_string()))?;

    let identity_pub_bytes = identity_keypair.public_key().serialize();
    let dht_key = Sha256::digest(&identity_pub_bytes).to_vec();

    let dht_record = DHTRecord::new(
        dht_key.clone(),
        secret_key_bytes,
        record.serialize()?,
        timestamp,
    )
    .await?;

    cmd_tx
        .send(SwarmCommand::PublishDht {
            key: dht_key,
            value: dht_record.serialize()?,
        })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    db.raw_write(
        TABLE_SETTINGS,
        "rendezvous_last_published",
        &timestamp.to_be_bytes(),
    )?;

    Ok(())
}

#[allow(clippy::cast_possible_truncation)]
pub async fn lookup_rendezvous(
    identity_pub: &[u8],
    dilithium_pub: &[u8],
    network: &NetworkManager,
) -> Result<Option<RendezvousRecord>> {
    let dht_key = Sha256::digest(identity_pub).to_vec();
    let (reply_tx, mut reply_rx) = mpsc::channel(16);

    network
        .primary
        .cmd_tx
        .send(SwarmCommand::FetchDht {
            key: dht_key.clone(),
            reply_tx,
        })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    let timeout = Duration::from_secs(30);

    let result = tokio::time::timeout(timeout, async {
        while let Some(bytes) = reply_rx.recv().await {
            if let Ok(dht_record) = DHTRecord::deserialize(&dht_key, &bytes, dilithium_pub)
                && let Ok(record) = RendezvousRecord::deserialize(&dht_record)
            {
                return Some(record);
            }
        }

        None
    })
    .await
    .ok()
    .flatten(); // No record found

    Ok(result)
}
