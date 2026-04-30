use crate::{KursalError, Result};
use keyring_core::{Entry, set_default_store};
use rand::{TryRngCore, rngs::OsRng};
use std::{collections::HashMap, path::Path};

pub struct KeychainConfig {
    pub storage_id: String,
    pub unsafe_write_key_to_file: bool,
}

const MASTER_SECRET_LEN: usize = 32;

pub fn init_keychain() -> Result<()> {
    let config = &HashMap::new();

    #[cfg(target_os = "android")]
    {
        use android_native_keyring_store::Store;
        set_default_store(
            Store::new_with_configuration(config)
                .map_err(|err| KursalError::Storage(err.to_string()))?,
        );
    }

    #[cfg(target_os = "macos")]
    {
        use apple_native_keyring_store::keychain::Store; // TODO: maybe change to protected
        set_default_store(
            Store::new_with_configuration(config)
                .map_err(|err| KursalError::Storage(err.to_string()))?,
        );
    }

    #[cfg(target_os = "ios")]
    {
        use apple_native_keyring_store::protected::Store;
        set_default_store(
            Store::new_with_configuration(config)
                .map_err(|err| KursalError::Storage(err.to_string()))?,
        );
    }

    #[cfg(target_os = "windows")]
    {
        use windows_native_keyring_store::Store;
        set_default_store(
            Store::new_with_configuration(config)
                .map_err(|err| KursalError::Storage(err.to_string()))?,
        );
    }

    #[cfg(target_os = "linux")]
    {
        use linux_keyutils_keyring_store::Store;
        set_default_store(
            Store::new_with_configuration(config)
                .map_err(|err| KursalError::Storage(err.to_string()))?,
        );
    }

    #[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
    {
        use dbus_secret_service_keyring_store::Store;
        set_default_store(
            Store::new_with_configuration(config)
                .map_err(|err| KursalError::Storage(err.to_string()))?,
        );
    }

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
    log::info!("keychain: Storing new master secret");
    if secret.len() != MASTER_SECRET_LEN {
        return Err(KursalError::Crypto(format!(
            "Master secret must be exactly {MASTER_SECRET_LEN} bytes"
        )));
    }

    let hex_secret = hex::encode(secret);

    if config.unsafe_write_key_to_file {
        log::warn!(
            "UNSAFE! Writing master secret to a file is DANGEROUS and should be used with care! Like for debugging purposes!"
        );
        let path = app_data_dir.join(format!("{}.key", config.storage_id));
        std::fs::write(path, hex_secret).map_err(|err| KursalError::Storage(err.to_string()))?;
    } else {
        let entry_ref = entry
            .as_ref()
            .ok_or_else(|| KursalError::Storage("Missing keychain entry".to_string()))?;

        // Guard: refuse to overwrite an existing entry
        match entry_ref.get_secret() {
            Ok(_) => {
                return Err(KursalError::Storage(
                    "Refusing to overwrite existing master secret in keychain".to_string(),
                ));
            }
            Err(keyring_core::Error::NoEntry) => {} // expected
            Err(err) => return Err(KursalError::Storage(err.to_string())),
        }

        entry_ref
            .set_secret(hex_secret.as_bytes())
            .map_err(|err| KursalError::Storage(err.to_string()))?;
    }

    Ok(())
}

pub fn load_master_secret(
    config: &KeychainConfig,
    entry: &Option<Entry>,
    app_data_dir: &Path,
) -> Result<Option<Vec<u8>>> {
    log::info!("keychain: Loading master secret");
    let raw_secret = if config.unsafe_write_key_to_file {
        log::warn!(
            "UNSAFE! Reading master secret to a file is DANGEROUS and should be used with care! Like for debugging purposes!"
        );
        let path = app_data_dir.join(format!("{}.key", config.storage_id));
        std::fs::read(path).ok()
    } else {
        let entry_ref = entry
            .as_ref()
            .ok_or_else(|| KursalError::Storage("Missing keychain entry".to_string()))?;

        match entry_ref.get_secret() {
            Ok(secret) => Some(secret),
            Err(keyring_core::Error::NoEntry) => None,
            Err(err) => return Err(KursalError::Storage(err.to_string())),
        }
    };

    if let Some(secret) = raw_secret {
        let decoded = hex::decode(&secret).map_err(|_| {
            KursalError::Crypto("Stored master secret is not valid hex".to_string())
        })?;

        if decoded.len() != MASTER_SECRET_LEN {
            return Err(KursalError::Crypto(format!(
                "Stored master secret has wrong length: expected {} bytes, got {}",
                MASTER_SECRET_LEN,
                decoded.len()
            )));
        }

        return Ok(Some(decoded));
    }

    Ok(None)
}

pub fn generate_master_secret() -> Result<[u8; 32]> {
    let mut result = [0u8; 32];

    let mut rng = OsRng;
    rng.try_fill_bytes(&mut result)
        .map_err(|err| KursalError::Crypto(err.to_string()))?;

    Ok(result)
}
