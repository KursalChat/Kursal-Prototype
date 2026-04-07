use crate::{
    KursalError, Result,
    crypto::dilithium::{dilithium_sign, dilithium_verify},
    network::kademlia::{KAD_MAX_AGE, KAD_MAX_PAYLOAD},
    storage::get_timestamp_secs,
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Serialize, Deserialize)]
pub struct DHTRecord {
    pub key: Vec<u8>,
    pub value: Vec<u8>,
    pub proof: u128,
    pub signature: Vec<u8>,
    pub timestamp: u64,
}

impl DHTRecord {
    pub async fn new(
        key: Vec<u8>,
        secret_key_bytes: Vec<u8>,
        value: Vec<u8>,
        timestamp: u64,
    ) -> Result<Self> {
        let mut to_sign = value.clone();
        to_sign.extend_from_slice(&timestamp.to_le_bytes());

        let signature = dilithium_sign(&secret_key_bytes, &to_sign)?;
        let proof = mine_pow_async(to_sign).await?;

        Ok(Self {
            key,
            value,
            signature,
            proof,
            timestamp,
        })
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|err| KursalError::Storage(err.to_string()))
    }

    pub fn deserialize(key: &[u8], bytes: &[u8], dilithium_pub: &[u8]) -> Result<Vec<u8>> {
        let record = DHTRecord::is_valid(key, bytes)?;

        let mut to_sign = record.value.clone();
        to_sign.extend_from_slice(&record.timestamp.to_le_bytes());

        if !dilithium_verify(dilithium_pub, &to_sign, &record.signature).unwrap_or(false) {
            return Err(KursalError::Crypto("Signature did not match".to_string()));
        }

        Ok(record.value)
    }

    pub fn is_valid(key: &[u8], bytes: &[u8]) -> Result<Self> {
        if bytes.len() > KAD_MAX_PAYLOAD {
            return Err(KursalError::Crypto(format!(
                "payload too large: {} bytes (max {} bytes)",
                bytes.len(),
                KAD_MAX_PAYLOAD
            )));
        }

        let record: Self =
            bincode::deserialize(bytes).map_err(|err| KursalError::Storage(err.to_string()))?;

        if record.key != key {
            return Err(KursalError::Crypto(
                "Record does not have the correct key".to_string(),
            ));
        }

        let now = get_timestamp_secs()?;

        if record.timestamp < now + KAD_MAX_AGE {
            return Err(KursalError::Crypto(
                "Timestamp in the record too old".to_string(),
            ));
        }

        if record.timestamp > now {
            return Err(KursalError::Crypto(
                "Timestamp in the record is in the future".to_string(),
            ));
        }

        let mut to_sign = record.value.clone();
        to_sign.extend_from_slice(&record.timestamp.to_le_bytes());

        if !check_pow(&to_sign, record.proof) {
            return Err(KursalError::Crypto("Proof of work is invalid".to_string()));
        }

        Ok(record)
    }
}

//

const DHT_TARGET: [u8; 32] = [
    0x00, 0x00, 0xA0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
];

pub async fn mine_pow_async(message: Vec<u8>) -> Result<u128> {
    tokio::task::spawn_blocking(move || mine_pow(&message))
        .await
        .map_err(|err| KursalError::Crypto(err.to_string()))?
}

pub fn mine_pow(message: &[u8]) -> Result<u128> {
    let threads = rayon::current_num_threads() as u128;

    (0..threads)
        .into_par_iter()
        .find_map_any(|thread_id| {
            let mut nonce = thread_id;
            loop {
                if check_pow(message, nonce) {
                    return Some(nonce);
                }
                nonce = nonce.checked_add(threads)?;
            }
        })
        .ok_or_else(|| KursalError::Crypto("Failed to find valid POW nonce".to_string()))
}

pub fn check_pow(message: &[u8], nonce: u128) -> bool {
    let mut hasher = Sha256::new();
    hasher.update(message);
    hasher.update(nonce.to_le_bytes());

    let hash: [u8; 32] = hasher.finalize().into();

    hash <= DHT_TARGET
}
