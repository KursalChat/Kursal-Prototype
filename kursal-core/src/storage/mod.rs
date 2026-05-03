use crate::{
    KursalError, Result,
    api::file_transfers::FileTransferEntry,
    apiserver::LocalApiConfig,
    contacts::Contact,
    crypto::stream::{stream_decrypt, stream_encrypt},
    identity::UserId,
    storage::filetransfer::{get_auto_download_storage_for, get_folder_size},
};
use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};
use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use async_trait::async_trait;
use hkdf::Hkdf;
use libsignal_protocol::{
    DeviceId, Direction, GenericSignedPreKey, IdentityChange, IdentityKey, IdentityKeyPair,
    IdentityKeyStore, KyberPreKeyId, KyberPreKeyRecord, KyberPreKeyStore, PreKeyId, PreKeyRecord,
    PreKeyStore, ProtocolAddress, PublicKey, SessionRecord, SessionStore, SignalProtocolError,
    SignedPreKeyId, SignedPreKeyRecord, SignedPreKeyStore,
};
use rand::{TryRngCore, rngs::OsRng};
use redb::{ReadableDatabase, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    array::TryFromSliceError,
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};
use tokio::sync::Mutex;
use zeroize::Zeroizing;

pub mod backup;
pub mod file;
pub mod filetransfer;

pub const TABLE_SESSIONS: TableDefinition<&str, &[u8]> = TableDefinition::new("sessions");
pub const TABLE_IDENTITY_KEYS: TableDefinition<&str, &[u8]> = TableDefinition::new("identity_keys");
pub const TABLE_PRE_KEYS: TableDefinition<&str, &[u8]> = TableDefinition::new("pre_keys");
pub const TABLE_SIGNED_PRE_KEYS: TableDefinition<&str, &[u8]> =
    TableDefinition::new("signed_pre_keys");
pub const TABLE_KYBER_PRE_KEYS: TableDefinition<&str, &[u8]> =
    TableDefinition::new("kyber_pre_keys");
pub const TABLE_CONTACTS: TableDefinition<&str, &[u8]> = TableDefinition::new("contacts");
pub const TABLE_MESSAGES: TableDefinition<&str, &[u8]> = TableDefinition::new("messages");
pub const TABLE_LTC_CACHE: TableDefinition<&str, &[u8]> = TableDefinition::new("ltc_cache");
pub const TABLE_SETTINGS: TableDefinition<&str, &[u8]> = TableDefinition::new("settings");
pub const TABLE_FILE_TRANSFERS: TableDefinition<&str, &[u8]> =
    TableDefinition::new("file_transfers");

pub struct Database {
    inner: redb::Database,
    key: Zeroizing<[u8; 32]>,
}

impl Database {
    pub fn open(path: &Path, key: [u8; 32]) -> Result<Self> {
        let db =
            redb::Database::create(path).map_err(|err| KursalError::Storage(err.to_string()))?;

        Ok(Self {
            inner: db,
            key: Zeroizing::new(key),
        })
    }

    pub(crate) fn raw_write(
        &self,
        table: TableDefinition<&str, &[u8]>,
        key: &str,
        value: &[u8],
    ) -> Result<Option<Vec<u8>>> {
        let write_txn = self
            .inner
            .begin_write()
            .map_err(|err| KursalError::Storage(err.to_string()))?;

        // Encrypt value with AES and nonce
        let enc_value = stream_encrypt(&self.key, value)?;

        // Save to DB
        let previous_value = {
            let mut table = write_txn
                .open_table(table)
                .map_err(|err| KursalError::Storage(err.to_string()))?;

            let previous_value = table
                .insert(key, enc_value.as_slice())
                .map_err(|err| KursalError::Storage(err.to_string()))?;

            previous_value
                .map(|v| stream_decrypt(&self.key, v.value()))
                .transpose()
        };

        write_txn
            .commit()
            .map_err(|err| KursalError::Storage(err.to_string()))?;

        previous_value
    }

    pub(crate) fn raw_read(
        &self,
        table: TableDefinition<&str, &[u8]>,
        key: &str,
    ) -> Result<Option<Vec<u8>>> {
        let read_txn = self
            .inner
            .begin_read()
            .map_err(|err| KursalError::Storage(err.to_string()))?;

        let table = match read_txn.open_table(table) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(err) => return Err(KursalError::Storage(err.to_string())),
        };

        let result = table
            .get(key)
            .map_err(|err| KursalError::Storage(err.to_string()))?;

        match result {
            None => Ok(None),
            Some(bytes) => {
                let decrypted = stream_decrypt(&self.key, bytes.value())?;
                Ok(Some(decrypted))
            }
        }
    }

    pub(crate) fn raw_delete(&self, table: TableDefinition<&str, &[u8]>, key: &str) -> Result<()> {
        let write_txn = self
            .inner
            .begin_write()
            .map_err(|err| KursalError::Storage(err.to_string()))?;

        {
            let mut table = write_txn
                .open_table(table)
                .map_err(|err| KursalError::Storage(err.to_string()))?;

            table
                .remove(key)
                .map_err(|err| KursalError::Storage(err.to_string()))?;
        }

        write_txn
            .commit()
            .map_err(|err| KursalError::Storage(err.to_string()))?;

        Ok(())
    }

    pub(crate) fn raw_range(
        &self,
        table: TableDefinition<&str, &[u8]>,
        start: &str,
        end: &str,
        limit: Option<usize>,
    ) -> Result<Vec<(String, Vec<u8>)>> {
        let read_txn = self
            .inner
            .begin_read()
            .map_err(|err| KursalError::Storage(err.to_string()))?;

        let table = match read_txn.open_table(table) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(vec![]),
            Err(err) => return Err(KursalError::Storage(err.to_string())),
        };

        let op = table
            .range(start..=end)
            .map_err(|err| KursalError::Storage(err.to_string()))?;

        let op: Box<dyn Iterator<Item = _>> = if let Some(limit) = limit {
            Box::new(op.take(limit))
        } else {
            Box::new(op)
        };

        op.map(|entry| {
            let (k, v) = entry.map_err(|err| KursalError::Storage(err.to_string()))?;

            let decrypted = stream_decrypt(&self.key, v.value())?;

            Ok((k.value().to_string(), decrypted))
        })
        .collect()
    }

    pub(crate) fn raw_readall(
        &self,
        table: TableDefinition<&str, &[u8]>,
    ) -> Result<Vec<(String, Vec<u8>)>> {
        let read_txn = self
            .inner
            .begin_read()
            .map_err(|err| KursalError::Storage(err.to_string()))?;

        let table = match read_txn.open_table(table) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(vec![]),
            Err(err) => return Err(KursalError::Storage(err.to_string())),
        };

        table
            .iter()
            .map_err(|err| KursalError::Storage(err.to_string()))?
            .map(|entry| {
                let (k, v) = entry.map_err(|err| KursalError::Storage(err.to_string()))?;

                let key = k.value().to_string();

                let bytes = v.value();
                let nonce: [u8; 12] = bytes[0..12]
                    .try_into()
                    .map_err(|e: TryFromSliceError| KursalError::Crypto(e.to_string()))?;

                let cipher = Aes256Gcm::new_from_slice(&*self.key)
                    .map_err(|err| KursalError::Crypto(err.to_string()))?;

                let decrypted = cipher
                    .decrypt(&nonce.into(), &bytes[12..])
                    .map_err(|err| KursalError::Crypto(err.to_string()))?;

                Ok((key, decrypted))
            })
            .collect::<Result<Vec<_>>>()
    }

    pub(crate) fn raw_delete_prefix(
        &self,
        table: TableDefinition<&str, &[u8]>,
        prefix: &str,
    ) -> Result<()> {
        let write_txn = self
            .inner
            .begin_write()
            .map_err(|err| KursalError::Storage(err.to_string()))?;

        {
            let mut tbl = write_txn
                .open_table(table)
                .map_err(|err| KursalError::Storage(err.to_string()))?;

            let keys_to_delete: Vec<String> = tbl
                .range(prefix..)
                .map_err(|err| KursalError::Storage(err.to_string()))?
                .take_while(|entry| {
                    entry
                        .as_ref()
                        .map(|(k, _)| k.value().starts_with(prefix))
                        .unwrap_or(false)
                })
                .map(|entry| entry.map(|(k, _)| k.value().to_string()))
                .collect::<std::result::Result<_, _>>()
                .map_err(|err| KursalError::Storage(err.to_string()))?;

            for key in &keys_to_delete {
                tbl.remove(key.as_str())
                    .map_err(|err| KursalError::Storage(err.to_string()))?;
            }
        }

        write_txn
            .commit()
            .map_err(|err| KursalError::Storage(err.to_string()))?;

        Ok(())
    }

    pub(crate) fn raw_delete_all(&self, table: TableDefinition<&str, &[u8]>) -> Result<()> {
        let write_txn = self
            .inner
            .begin_write()
            .map_err(|err| KursalError::Storage(err.to_string()))?;

        {
            let mut tbl = write_txn
                .open_table(table)
                .map_err(|err| KursalError::Storage(err.to_string()))?;

            let keys_to_delete: Vec<String> = tbl
                .iter()
                .map_err(|err| KursalError::Storage(err.to_string()))?
                .map(|entry| entry.map(|(k, _)| k.value().to_string()))
                .collect::<std::result::Result<_, _>>()
                .map_err(|err| KursalError::Storage(err.to_string()))?;

            for key in &keys_to_delete {
                tbl.remove(key.as_str())
                    .map_err(|err| KursalError::Storage(err.to_string()))?;
            }
        }

        write_txn
            .commit()
            .map_err(|err| KursalError::Storage(err.to_string()))?;

        Ok(())
    }
}

#[derive(Clone)]
pub struct SharedDatabase(pub Arc<Mutex<Database>>);
impl SharedDatabase {
    pub fn from_db(db: Database) -> SharedDatabase {
        SharedDatabase(Arc::new(Mutex::new(db)))
    }

    pub async fn read_session(&self, address: &ProtocolAddress) -> Result<Option<SessionRecord>> {
        let bytes = self
            .0
            .lock()
            .await
            .raw_read(TABLE_SESSIONS, &address.to_string())?;

        match bytes {
            None => Ok(None),
            Some(b) => Ok(Some(
                SessionRecord::deserialize(&b)
                    .map_err(|err| KursalError::Storage(err.to_string()))?,
            )),
        }
    }
}

#[async_trait(?Send)]
impl SessionStore for SharedDatabase {
    async fn load_session(
        &self,
        address: &ProtocolAddress,
    ) -> libsignal_protocol::error::Result<Option<SessionRecord>> {
        let bytes = self
            .0
            .lock()
            .await
            .raw_read(TABLE_SESSIONS, &address.to_string())
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        match bytes {
            None => Ok(None),
            Some(b) => {
                let parsed = SessionRecord::deserialize(&b)?;
                Ok(Some(parsed))
            }
        }
    }

    async fn store_session(
        &mut self,
        address: &ProtocolAddress,
        record: &SessionRecord,
    ) -> libsignal_protocol::error::Result<()> {
        let bytes = record.serialize()?;

        self.0
            .lock()
            .await
            .raw_write(TABLE_SESSIONS, &address.to_string(), &bytes)
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        Ok(())
    }
}

#[async_trait(?Send)]
impl IdentityKeyStore for SharedDatabase {
    async fn get_identity_key_pair(&self) -> libsignal_protocol::error::Result<IdentityKeyPair> {
        let bytes = self
            .0
            .lock()
            .await
            .raw_read(TABLE_IDENTITY_KEYS, "local_identity")
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        match bytes {
            None => Err(SignalProtocolError::InvalidArgument(
                "Identity Key Pair not found".to_string(),
            )),
            Some(b) => {
                let parsed = IdentityKeyPair::try_from(b.as_slice())?;

                Ok(parsed)
            }
        }
    }

    async fn get_local_registration_id(&self) -> libsignal_protocol::error::Result<u32> {
        let bytes = self
            .0
            .lock()
            .await
            .raw_read(TABLE_SETTINGS, "registration_id")
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        match bytes {
            None => Err(SignalProtocolError::InvalidArgument(
                "Local Registration ID not found".to_string(),
            )),
            Some(b) => {
                let parsed =
                    u32::from_be_bytes(b[0..4].try_into().map_err(|e: TryFromSliceError| {
                        SignalProtocolError::InvalidArgument(e.to_string())
                    })?);

                Ok(parsed)
            }
        }
    }

    async fn save_identity(
        &mut self,
        address: &ProtocolAddress,
        identity: &IdentityKey,
    ) -> libsignal_protocol::error::Result<IdentityChange> {
        let bytes = identity.serialize().to_vec();

        let previous = self
            .0
            .lock()
            .await
            .raw_write(TABLE_IDENTITY_KEYS, &address.to_string(), &bytes)
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        Ok(match previous {
            None => IdentityChange::NewOrUnchanged,
            Some(prev) => {
                if prev == bytes {
                    IdentityChange::NewOrUnchanged
                } else {
                    IdentityChange::ReplacedExisting
                }
            }
        })
    }

    async fn is_trusted_identity(
        &self,
        address: &ProtocolAddress,
        identity: &IdentityKey,
        _direction: Direction,
    ) -> libsignal_protocol::error::Result<bool> {
        let found = self.get_identity(address).await?;

        match found {
            None => Ok(true),
            Some(key) => {
                if &key == identity {
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        }
    }

    async fn get_identity(
        &self,
        address: &ProtocolAddress,
    ) -> libsignal_protocol::error::Result<Option<IdentityKey>> {
        let bytes = self
            .0
            .lock()
            .await
            .raw_read(TABLE_IDENTITY_KEYS, &address.to_string())
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        match bytes {
            None => Ok(None),
            Some(b) => {
                let parsed = IdentityKey::decode(&b)?;
                Ok(Some(parsed))
            }
        }
    }
}

#[async_trait(?Send)]
impl PreKeyStore for SharedDatabase {
    async fn get_pre_key(
        &self,
        prekey_id: PreKeyId,
    ) -> libsignal_protocol::error::Result<PreKeyRecord> {
        let key = format!("prekey_{}", u32::from(prekey_id));

        let bytes = self
            .0
            .lock()
            .await
            .raw_read(TABLE_PRE_KEYS, &key)
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        match bytes {
            None => Err(SignalProtocolError::InvalidArgument(
                "Pre Key not found".to_string(),
            )),
            Some(b) => {
                let parsed = PreKeyRecord::deserialize(&b)?;

                Ok(parsed)
            }
        }
    }

    async fn save_pre_key(
        &mut self,
        prekey_id: PreKeyId,
        record: &PreKeyRecord,
    ) -> libsignal_protocol::error::Result<()> {
        let key = format!("prekey_{}", u32::from(prekey_id));
        let bytes = record.serialize()?;

        self.0
            .lock()
            .await
            .raw_write(TABLE_PRE_KEYS, &key, &bytes)
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        Ok(())
    }

    async fn remove_pre_key(
        &mut self,
        prekey_id: PreKeyId,
    ) -> libsignal_protocol::error::Result<()> {
        let key = format!("prekey_{}", u32::from(prekey_id));

        self.0
            .lock()
            .await
            .raw_delete(TABLE_PRE_KEYS, &key)
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        Ok(())
    }
}

#[async_trait(?Send)]
impl SignedPreKeyStore for SharedDatabase {
    async fn get_signed_pre_key(
        &self,
        signed_prekey_id: SignedPreKeyId,
    ) -> libsignal_protocol::error::Result<SignedPreKeyRecord> {
        let key = format!("signed_prekey_{}", u32::from(signed_prekey_id));

        let bytes = self
            .0
            .lock()
            .await
            .raw_read(TABLE_SIGNED_PRE_KEYS, &key)
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        match bytes {
            None => Err(SignalProtocolError::InvalidArgument(
                "Signed Pre Key not found".to_string(),
            )),
            Some(b) => {
                let parsed = SignedPreKeyRecord::deserialize(&b)?;

                Ok(parsed)
            }
        }
    }

    async fn save_signed_pre_key(
        &mut self,
        signed_prekey_id: SignedPreKeyId,
        record: &SignedPreKeyRecord,
    ) -> libsignal_protocol::error::Result<()> {
        let key = format!("signed_prekey_{}", u32::from(signed_prekey_id));
        let bytes = record.serialize()?;

        self.0
            .lock()
            .await
            .raw_write(TABLE_SIGNED_PRE_KEYS, &key, &bytes)
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        Ok(())
    }
}

#[async_trait(?Send)]
impl KyberPreKeyStore for SharedDatabase {
    async fn get_kyber_pre_key(
        &self,
        kyber_prekey_id: KyberPreKeyId,
    ) -> libsignal_protocol::error::Result<KyberPreKeyRecord> {
        let key = format!("kyber_prekey_{}", u32::from(kyber_prekey_id));

        let bytes = self
            .0
            .lock()
            .await
            .raw_read(TABLE_KYBER_PRE_KEYS, &key)
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        match bytes {
            None => Err(SignalProtocolError::InvalidArgument(
                "Kyber Pre Key not found".to_string(),
            )),
            Some(b) => {
                let parsed = KyberPreKeyRecord::deserialize(&b)?;

                Ok(parsed)
            }
        }
    }

    async fn save_kyber_pre_key(
        &mut self,
        kyber_prekey_id: KyberPreKeyId,
        record: &KyberPreKeyRecord,
    ) -> libsignal_protocol::error::Result<()> {
        let key = format!("kyber_prekey_{}", u32::from(kyber_prekey_id));
        let bytes = record.serialize()?;

        self.0
            .lock()
            .await
            .raw_write(TABLE_KYBER_PRE_KEYS, &key, &bytes)
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        Ok(())
    }

    async fn mark_kyber_pre_key_used(
        &mut self,
        kyber_prekey_id: KyberPreKeyId,
        _ec_prekey_id: SignedPreKeyId,
        _base_key: &PublicKey,
    ) -> libsignal_protocol::error::Result<()> {
        let key = format!("kyber_prekey_{}", u32::from(kyber_prekey_id));

        self.0
            .lock()
            .await
            .raw_delete(TABLE_KYBER_PRE_KEYS, &key)
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        Ok(())
    }
}

pub fn delete_message_history_for(db: &Database, contact_id: String) -> Result<()> {
    db.raw_delete_prefix(TABLE_MESSAGES, &format!("{contact_id}:"))
}

pub fn delete_message_history_all(db: &Database) -> Result<()> {
    db.raw_delete_all(TABLE_MESSAGES)
}

pub fn get_local_identity_pub(db: &Database) -> Result<Vec<u8>> {
    let identity_bytes = db
        .raw_read(TABLE_IDENTITY_KEYS, "local_identity")?
        .ok_or(KursalError::Storage("No local identity".to_string()))?;

    let identity_keypair = IdentityKeyPair::try_from(identity_bytes.as_slice())
        .map_err(|e| KursalError::Identity(e.to_string()))?;

    Ok(identity_keypair.public_key().serialize().to_vec())
}

pub fn get_local_user_id(db: &Database) -> Result<UserId> {
    let identity_pub_key = get_local_identity_pub(db)?;
    let user_id: [u8; 32] = Sha256::digest(&identity_pub_key).into();

    Ok(UserId(user_id))
}

pub fn get_local_address(db: &Database) -> Result<ProtocolAddress> {
    let user_id = get_local_user_id(db)?;

    Ok(ProtocolAddress::new(
        hex::encode(user_id.0),
        DeviceId::new(1u8).unwrap(),
    ))
}

pub fn get_dilithium_pub(db: &Database) -> Result<Vec<u8>> {
    db.raw_read(TABLE_SETTINGS, "dilithium_public")?
        .ok_or(KursalError::Storage("No dilithium public key".to_string()))
}

//

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactUsage {
    pub contact_id: String,
    pub db_bytes: u64,
    pub files_bytes: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageUsage {
    pub logs_bytes: u64,
    pub db_bytes: u64,
    pub files_bytes: u64,
    pub per_contact: Vec<ContactUsage>,
}
impl StorageUsage {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|err| KursalError::Storage(err.to_string()))
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(|err| KursalError::Storage(err.to_string()))
    }
}

pub fn get_storage_usage(
    db: &Database,
    logs_dir: PathBuf,
    cache_dir: PathBuf,
    db_path: PathBuf,
) -> Result<StorageUsage> {
    let logs_bytes = get_folder_size(logs_dir, 1)?;
    let db_bytes = db_path
        .metadata()
        .map(|m| m.len())
        .map_err(KursalError::Io)?;

    let mut files_bytes = 0u64;

    let contacts = Contact::load_all(db)?;
    let mut per_contact = Vec::with_capacity(contacts.len());
    for contact in contacts.into_iter() {
        let contact_id = hex::encode(contact.user_id.0);

        let contact_files_bytes =
            get_auto_download_storage_for(cache_dir.clone(), contact_id.clone())?;

        let read_txn = db
            .inner
            .begin_read()
            .map_err(|err| KursalError::Storage(err.to_string()))?;

        let table = match read_txn.open_table(TABLE_MESSAGES) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => {
                per_contact.push(ContactUsage {
                    contact_id,
                    db_bytes: 0u64,
                    files_bytes: contact_files_bytes,
                });
                continue;
            }
            Err(err) => return Err(KursalError::Storage(err.to_string())),
        };

        let prefix = format!("{contact_id}:");
        let op = table
            .range(prefix.as_str()..)
            .map_err(|err| KursalError::Storage(err.to_string()))?;

        let mut db_bytes_sum = 0u64;
        for entry in op {
            let (k, v) = entry.map_err(|err| KursalError::Storage(err.to_string()))?;
            if !k.value().starts_with(&prefix) {
                break;
            }
            db_bytes_sum += v.value().len() as u64;
        }

        per_contact.push(ContactUsage {
            contact_id,
            db_bytes: db_bytes_sum,
            files_bytes: contact_files_bytes,
        });
        files_bytes += contact_files_bytes;
    }

    Ok(StorageUsage {
        logs_bytes,
        db_bytes,
        files_bytes,
        per_contact,
    })
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoAcceptConfig {
    pub mode: String, // "nobody" | "verified" | "all"
    pub size_cap_bytes: u64,
}
impl AutoAcceptConfig {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|err| KursalError::Storage(err.to_string()))
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(|err| KursalError::Storage(err.to_string()))
    }
}

pub fn get_auto_accept_config(db: &Database) -> AutoAcceptConfig {
    let default = AutoAcceptConfig {
        mode: "nobody".to_string(),
        size_cap_bytes: 2 * 1024 * 1024,
    };
    match db.raw_read(TABLE_SETTINGS, "auto_accept_config") {
        Ok(Some(bytes)) => AutoAcceptConfig::deserialize(&bytes).unwrap_or(default),
        _ => default,
    }
}

pub fn set_auto_accept_config(db: &Database, config: AutoAcceptConfig) -> Result<()> {
    let bytes = config.serialize()?;

    db.raw_write(TABLE_SETTINGS, "auto_accept_config", &bytes)?;

    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoDownloadConfig {
    pub scope: String, // "per_contact" | "all_contacts"
    pub limit_bytes: u64,
}
impl AutoDownloadConfig {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|err| KursalError::Storage(err.to_string()))
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(|err| KursalError::Storage(err.to_string()))
    }
}

pub fn get_auto_download_config(db: &Database) -> AutoDownloadConfig {
    let default = AutoDownloadConfig {
        scope: "all_contacts".to_string(),
        limit_bytes: 100 * 1024 * 1024,
    };

    match db.raw_read(TABLE_SETTINGS, "auto_download_config") {
        Ok(Some(bytes)) => AutoDownloadConfig::deserialize(&bytes).unwrap_or(default),
        _ => default,
    }
}

pub fn set_auto_download_config(db: &Database, config: AutoDownloadConfig) -> Result<()> {
    let bytes = config.serialize()?;

    db.raw_write(TABLE_SETTINGS, "auto_download_config", &bytes)?;

    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedFileEntry {
    pub id: String,
    pub filepath: String,
    pub size_bytes: u64,
    pub recipient_id: String,
    pub shared_at: u64,
    pub last_accessed_at: Option<u64>,
}
impl SharedFileEntry {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|err| KursalError::Storage(err.to_string()))
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(|err| KursalError::Storage(err.to_string()))
    }
}

pub fn files_list_shared(db: &Database) -> Result<Vec<SharedFileEntry>> {
    let get_all = db
        .raw_range(TABLE_FILE_TRANSFERS, "send:", "send;", None)?
        .into_iter()
        .filter_map(|(key, bytes)| {
            if let Ok(entry) = FileTransferEntry::deserialize(&bytes) {
                Some((key, entry))
            } else {
                None
            }
        })
        .map(|(key, entry)| SharedFileEntry {
            id: key.clone(),
            filepath: entry.path,
            size_bytes: entry.size_bytes,
            recipient_id: key.split(':').nth(1usize).unwrap_or("unknown").to_string(),
            shared_at: entry.shared_at,
            last_accessed_at: entry.last_accessed_at,
        })
        .collect();

    Ok(get_all)
}
pub fn files_revoke_shared(db: &Database, id: String) -> Result<()> {
    db.raw_delete(TABLE_FILE_TRANSFERS, &id)?;

    Ok(())
}

pub fn api_server_password(db: &Database) -> Result<String> {
    match db.raw_read(TABLE_SETTINGS, "api_server_password")? {
        None => Err(KursalError::Storage("Entry not found".to_string())),
        Some(bytes) => {
            String::from_utf8(bytes).map_err(|err| KursalError::Storage(err.to_string()))
        }
    }
}
pub fn set_new_api_server_password(db: &Database) -> Result<String> {
    let mut bytes = [0u8; 32];
    OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|err| KursalError::Crypto(err.to_string()))?;

    let token = hex::encode(bytes);

    let salt = SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
    let argon2 = Argon2::default();

    let password_hash = argon2
        .hash_password(token.as_bytes(), &salt)
        .map_err(|err| KursalError::Crypto(err.to_string()))?
        .to_string();

    db.raw_write(
        TABLE_SETTINGS,
        "api_server_password",
        password_hash.as_bytes(),
    )?;

    Ok(token)
}

pub fn api_server_config(db: &Database) -> Result<LocalApiConfig> {
    match db.raw_read(TABLE_SETTINGS, "api_server_config")? {
        None => Err(KursalError::Storage("Entry not found".to_string())),
        Some(bytes) => LocalApiConfig::deserialize(&bytes),
    }
}
pub fn set_api_server_config(db: &Database, config: LocalApiConfig) -> Result<()> {
    db.raw_write(
        TABLE_SETTINGS,
        "api_server_config",
        &config
            .serialize()
            .map_err(|err| KursalError::Storage(err.to_string()))?,
    )?;

    Ok(())
}

//

pub fn get_peer_rotation_interval(db: &Database) -> u64 {
    let default = 30 * 60 * 60;

    match db.raw_read(TABLE_SETTINGS, "peer_rotation_interval_secs") {
        Ok(Some(bytes)) => bytes.try_into().map(u64::from_be_bytes).unwrap_or(default),
        _ => default,
    }
}
pub fn set_peer_rotation_interval(db: &Database, value: u64) -> Result<()> {
    db.raw_write(
        TABLE_SETTINGS,
        "peer_rotation_interval_secs",
        &value.to_be_bytes(),
    )?;

    Ok(())
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RelayConfig {
    pub enabled: bool,
    pub max_connections: u32,
    pub max_connections_per_ip: u32,
}
impl RelayConfig {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|err| KursalError::Storage(err.to_string()))
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(|err| KursalError::Storage(err.to_string()))
    }
}
pub fn get_relay_config(db: &Database) -> RelayConfig {
    let default = RelayConfig {
        enabled: true,
        max_connections: 100u32,
        max_connections_per_ip: 3u32,
    };

    match db.raw_read(TABLE_SETTINGS, "relay_config") {
        Ok(Some(bytes)) => RelayConfig::deserialize(&bytes).unwrap_or(default),
        _ => default,
    }
}
pub fn set_relay_config(db: &Database, value: RelayConfig) -> Result<()> {
    let bytes = value.serialize()?;

    db.raw_write(TABLE_SETTINGS, "relay_config", &bytes)?;

    Ok(())
}

pub fn get_swarm_listening_port(db: &Database) -> Option<u16> {
    if cfg!(debug_assertions) {
        return None;
    }

    db.raw_read(TABLE_SETTINGS, "swarm_listening_port")
        .ok()
        .flatten()
        .and_then(|b| b.try_into().ok().map(u16::from_be_bytes))
}
pub fn set_swarm_listening_port(db: &Database, port: Option<u16>) -> Result<()> {
    if let Some(port) = port {
        db.raw_write(TABLE_SETTINGS, "swarm_listening_port", &port.to_be_bytes())?;
    } else {
        db.raw_delete(TABLE_SETTINGS, "swarm_listening_port")?;
    }

    Ok(())
}

pub fn get_swarm_mdns_enabled(db: &Database) -> bool {
    let default = true;

    match db.raw_read(TABLE_SETTINGS, "swarm_mdns_enabled") {
        Ok(Some(bytes)) => bytes == vec![1u8],
        _ => default,
    }
}
pub fn set_swarm_mdns_enabled(db: &Database, value: bool) -> Result<()> {
    db.raw_write(
        TABLE_SETTINGS,
        "swarm_mdns_enabled",
        if value { &[1u8] } else { &[0u8] },
    )?;

    Ok(())
}

pub fn get_updater_enabled(db: &Database) -> bool {
    let default = true;

    match db.raw_read(TABLE_SETTINGS, "updater_enabled") {
        Ok(Some(bytes)) => bytes == vec![1u8],
        _ => default,
    }
}
pub fn set_updater_enabled(db: &Database, value: bool) -> Result<()> {
    db.raw_write(
        TABLE_SETTINGS,
        "updater_enabled",
        if value { &[1u8] } else { &[0u8] },
    )?;

    Ok(())
}

pub fn get_typing_indicators_enabled(db: &Database) -> bool {
    let default = true;

    match db.raw_read(TABLE_SETTINGS, "typing_indicators_enabled") {
        Ok(Some(bytes)) => bytes == vec![0u8],
        _ => default,
    }
}
pub fn set_typing_indicators_enabled(db: &Database, value: bool) -> Result<()> {
    db.raw_write(
        TABLE_SETTINGS,
        "typing_indicators_enabled",
        if value { &[1u8] } else { &[0u8] },
    )?;

    Ok(())
}

//

pub fn get_local_profile(db: &Database) -> (String, Option<Vec<u8>>) {
    let default_username = "You".to_string();
    let username = match db.raw_read(TABLE_SETTINGS, "local_profile_username") {
        Ok(Some(bytes)) => std::str::from_utf8(&bytes)
            .map(|v| v.to_string())
            .unwrap_or(default_username),
        _ => default_username,
    };

    let default_avatar = None;
    let avatar = match db.raw_read(TABLE_SETTINGS, "local_profile_avatar") {
        Ok(Some(bytes)) => Some(bytes),
        _ => default_avatar,
    };

    (username, avatar)
}
pub fn set_local_profile(
    db: &Database,
    display_name: String,
    avatar_bytes: Option<Vec<u8>>,
) -> Result<()> {
    db.raw_write(
        TABLE_SETTINGS,
        "local_profile_username",
        display_name.as_bytes(),
    )?;

    db.raw_write(
        TABLE_SETTINGS,
        "local_profile_avatar",
        &avatar_bytes.unwrap_or(Vec::with_capacity(0)),
    )?;

    Ok(())
}

//

pub fn derive_db_key(secret: &[u8]) -> Result<[u8; 32]> {
    let hk = Hkdf::<Sha256>::new(None, secret);
    let mut key = [0u8; 32];

    hk.expand(b"kursal-db-key", &mut key)
        .map_err(|err| KursalError::Crypto(err.to_string()))?;

    Ok(key)
}

//

#[allow(clippy::cast_possible_truncation)]
pub fn get_timestamp_millis() -> Result<u64> {
    SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|el| el.as_millis() as u64)
        .map_err(|err| KursalError::Crypto(err.to_string()))
}

pub fn get_timestamp_secs() -> Result<u64> {
    SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|el| el.as_secs())
        .map_err(|err| KursalError::Crypto(err.to_string()))
}

//

// ooh spooky
pub const RESET_FULL_APP: &str = "SET_THIS_TAG_TO_RESET_THE_FULL_APP yeah its dangerous";
pub fn reset_full_app(db: &Database) -> Result<()> {
    db.raw_write(TABLE_SETTINGS, RESET_FULL_APP, RESET_FULL_APP.as_bytes())?;
    Ok(())
}
pub fn should_reset_full_app(db: &Database) -> bool {
    let val = db.raw_read(TABLE_SETTINGS, RESET_FULL_APP);

    match val {
        Ok(Some(bytes)) => bytes == RESET_FULL_APP.as_bytes(),
        _ => false,
    }
}
