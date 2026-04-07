use crate::{KursalError, Result, crypto::PreKeyBundleData, storage::SharedDatabase};
use libsignal_protocol::{
    PreKeySignalMessage, ProtocolAddress, PublicKey as SignalPublicKey, SignalMessage,
    message_decrypt_prekey, message_decrypt_signal, message_encrypt,
};
use rand::{TryRngCore, rngs::OsRng};
use std::time::SystemTime;

pub struct InitialMessage {
    pub registration_id: u32,
    pub pre_key_bundle: PreKeyBundleData,
    pub base_key_public: SignalPublicKey,
    pub ciphertext: Vec<u8>,
}

pub async fn message_send(
    db: SharedDatabase,
    remote_address: &ProtocolAddress,
    plaintext: &[u8],
) -> Result<Vec<u8>> {
    let mut rng = OsRng.unwrap_err();
    let encrypted = message_encrypt(
        plaintext,
        remote_address,
        &mut db.clone(),
        &mut db.clone(),
        SystemTime::now(),
        &mut rng,
    )
    .await
    .map_err(|err| KursalError::Crypto(err.to_string()))?;

    Ok(encrypted.serialize().to_vec())
}

pub async fn message_receive(
    db: SharedDatabase,
    remote_address: &ProtocolAddress,
    ciphertext: &[u8],
) -> Result<Vec<u8>> {
    if let Ok(msg) = SignalMessage::try_from(ciphertext) {
        let mut rng = OsRng.unwrap_err();
        let decrypted = message_decrypt_signal(
            &msg,
            remote_address,
            &mut db.clone(),
            &mut db.clone(),
            &mut rng,
        )
        .await
        .map_err(|err| KursalError::Crypto(err.to_string()))?;

        Ok(decrypted)
    } else if let Ok(msg) = PreKeySignalMessage::try_from(ciphertext) {
        let mut rng = OsRng.unwrap_err();
        let decrypted = message_decrypt_prekey(
            &msg,
            remote_address,
            &mut db.clone(),
            &mut db.clone(),
            &mut db.clone(),
            &db.clone(),
            &mut db.clone(),
            &mut rng,
        )
        .await
        .map_err(|err| KursalError::Crypto(err.to_string()))?;

        Ok(decrypted)
    } else {
        Err(KursalError::Crypto("Unknown message type".to_string()))
    }
}
