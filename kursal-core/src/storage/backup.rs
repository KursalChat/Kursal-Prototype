use crate::{
    KursalError, Result,
    identity::keychain::{KeychainConfig, get_entry, load_master_secret},
    storage::file::KursalFile,
};
use aes_gcm::{KeyInit, aead::Aead};
use argon2::Argon2;
use chacha20poly1305::ChaCha20Poly1305;
use rand::{TryRngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs::{read, write};

#[derive(Serialize, Deserialize)]
pub struct KursalBackupFile {
    pub version: u8,
    pub ciphertext: Vec<u8>,
    pub salt: [u8; 16],
    pub nonce: Vec<u8>,
}
impl KursalBackupFile {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|err| KursalError::Storage(err.to_string()))
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(|err| KursalError::Storage(err.to_string()))
    }
}

#[derive(Serialize, Deserialize)]
pub struct KursalBackup {
    pub master_key: Vec<u8>,
    pub database: Vec<u8>,
}
impl KursalBackup {
    pub fn serialize(&self, password: Vec<u8>) -> Result<Vec<u8>> {
        let plaintext =
            bincode::serialize(self).map_err(|err| KursalError::Storage(err.to_string()))?;

        let mut salt = [0u8; 16];
        OsRng
            .try_fill_bytes(&mut salt)
            .map_err(|err| KursalError::Crypto(err.to_string()))?;

        let mut key = [0u8; 32];
        Argon2::default()
            .hash_password_into(&password, &salt, &mut key)
            .map_err(|err| KursalError::Crypto(err.to_string()))?;

        let mut nonce_bytes = [0u8; 12];
        OsRng
            .try_fill_bytes(&mut nonce_bytes)
            .map_err(|err| KursalError::Crypto(err.to_string()))?;
        let nonce = chacha20poly1305::Nonce::from_slice(&nonce_bytes);

        let cipher = ChaCha20Poly1305::new_from_slice(&key)
            .map_err(|err| KursalError::Crypto(err.to_string()))?;
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_ref())
            .map_err(|err| KursalError::Crypto(err.to_string()))?;

        KursalBackupFile {
            ciphertext,
            salt,
            nonce: nonce.to_vec(),
            version: 0u8,
        }
        .serialize()
    }
    pub fn deserialize(bytes: Vec<u8>, password: Vec<u8>) -> Result<Self> {
        let file = KursalBackupFile::deserialize(&bytes)?;

        let mut key = [0u8; 32];
        Argon2::default()
            .hash_password_into(&password, &file.salt, &mut key)
            .map_err(|err| KursalError::Crypto(err.to_string()))?;

        let cipher = ChaCha20Poly1305::new_from_slice(&key)
            .map_err(|err| KursalError::Crypto(err.to_string()))?;
        let nonce = chacha20poly1305::Nonce::from_slice(&file.nonce);

        let plaintext = cipher
            .decrypt(nonce, file.ciphertext.as_ref())
            .map_err(|_| KursalError::Crypto("Wrong password".to_string()))?;

        bincode::deserialize(&plaintext).map_err(|err| KursalError::Storage(err.to_string()))
    }
}

pub async fn generate_backup(
    password: String,
    keychain_config: &KeychainConfig,
    app_data_dir: &Path,
    database_path: &Path,
) -> Result<Vec<u8>> {
    let entry = get_entry(keychain_config)?;
    let master_key = load_master_secret(keychain_config, &entry, app_data_dir)?.ok_or(
        KursalError::Identity("Could not get master secret".to_string()),
    )?;

    let database = read(database_path).await.map_err(KursalError::Io)?;

    let backup = KursalBackup {
        master_key,
        database,
    };

    KursalFile::Backup(backup.serialize(password.into_bytes())?).serialize()
}

pub async fn load_backup(
    password: String,
    bytes: Vec<u8>,
    database_path: &Path,
    keychain_config: &KeychainConfig,
    app_data_dir: &Path,
) -> Result<()> {
    let kursal_file = KursalFile::deserialize(&bytes)?;
    let bytes = match kursal_file {
        KursalFile::Backup(b) => b,
        _ => {
            return Err(KursalError::Storage(
                "Invalid file. Not a Kursal Backup".to_string(),
            ));
        }
    };

    let backup = KursalBackup::deserialize(bytes, password.into_bytes())?;

    write(database_path, backup.database)
        .await
        .map_err(KursalError::Io)?;

    let hex_secret = hex::encode(&backup.master_key);

    if keychain_config.unsafe_write_key_to_file {
        let path = app_data_dir.join(format!("{}.key", keychain_config.storage_id));
        std::fs::write(path, hex_secret).map_err(|err| KursalError::Storage(err.to_string()))?;
    } else {
        let entry = get_entry(keychain_config)?;

        let entry_ref = entry
            .as_ref()
            .ok_or_else(|| KursalError::Storage("Missing keychain entry".to_string()))?;

        entry_ref
            .set_secret(hex_secret.as_bytes())
            .map_err(|err| KursalError::Storage(err.to_string()))?;
    }

    Ok(())
}
