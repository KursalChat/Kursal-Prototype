use crate::{KursalError, Result};
use keyring_core::Entry;
use rand::{TryRngCore, rngs::OsRng};
use std::path::Path;

pub struct KeychainConfig {
    pub storage_id: String,
    pub unsafe_write_key_to_file: bool,
}

const MASTER_SECRET_LEN: usize = 32;

pub fn init_keychain() -> Result<()> {
    keyring::use_native_store(false).map_err(|err| KursalError::Storage(err.to_string()))?;

    Ok(())
}

pub fn get_entry(config: &KeychainConfig) -> Result<Option<Entry>> {
    if config.unsafe_write_key_to_file {
        Ok(None)
    } else {
        Ok(Some(
            keyring_core::Entry::new("kursal", &config.storage_id)
                .map_err(|err| KursalError::Storage(err.to_string()))?,
        ))
    }
}

pub fn store_master_secret(
    secret: &[u8],
    config: &KeychainConfig,
    entry: &Option<Entry>,
    app_data_dir: &Path,
) -> Result<()> {
    if secret.len() < MASTER_SECRET_LEN {
        return Err(KursalError::Crypto(format!(
            "Master secret must be at least {MASTER_SECRET_LEN} bytes"
        )));
    }

    if config.unsafe_write_key_to_file {
        log::warn!(
            "UNSAFE! Writing master secret to a file is DANGEROUS and should be used with care! Like for debugging purposes!"
        );
        let path = app_data_dir.join(format!("{}.key", config.storage_id));

        std::fs::write(path, secret).map_err(|err| KursalError::Storage(err.to_string()))?;
    } else {
        // TODO: determine if it's needed or not
        // let _ = entry.delete_credential();
        if let Err(err) = entry
            .as_ref()
            .expect("Entry must be defined when write_to_file is false")
            .set_secret(secret)
        {
            // TODO: use other method instead?
            return Err(KursalError::Storage(err.to_string()));
        }
    }

    Ok(())
}

pub fn load_master_secret(
    config: &KeychainConfig,
    entry: &Option<Entry>,
    app_data_dir: &Path,
) -> Result<Option<Vec<u8>>> {
    if config.unsafe_write_key_to_file {
        log::warn!(
            "UNSAFE! Reading master secret to a file is DANGEROUS and should be used with care! Like for debugging purposes!"
        );
        let path = app_data_dir.join(format!("{}.key", config.storage_id));

        let out = std::fs::read(path)
            .map_err(|err| KursalError::Storage(err.to_string()))
            .ok();

        Ok(out)
    } else {
        match entry
            .as_ref()
            .expect("Entry must be defined when write_to_file is false")
            .get_secret()
        {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(err) => Err(KursalError::Storage(err.to_string())),
        }
    }
}

pub fn generate_master_secret() -> Result<[u8; 32]> {
    let mut result = [0u8; 32];

    let mut rng = OsRng;
    rng.try_fill_bytes(&mut result)
        .map_err(|err| KursalError::Crypto(err.to_string()))?;

    Ok(result)
}
