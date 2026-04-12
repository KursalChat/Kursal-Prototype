use crate::messaging::enums::KeyRotation;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub mod ltc;
pub mod nearby;
pub mod otp;

#[derive(Serialize, Deserialize, Clone)]
pub struct ContactResponse {
    pub pre_key_bundle: Vec<u8>,
    pub peer_id: String,
    pub dilithium_pub_key: Vec<u8>,
    pub relay_addresses: Vec<String>,
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
