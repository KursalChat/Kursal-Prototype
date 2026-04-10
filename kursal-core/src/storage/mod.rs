use crate::{
    KursalError, Result,
    crypto::stream::{stream_decrypt, stream_encrypt},
    identity::UserId,
};
use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};
use async_trait::async_trait;
use hkdf::Hkdf;
use libsignal_protocol::{
    Direction, GenericSignedPreKey, IdentityChange, IdentityKey, IdentityKeyPair, IdentityKeyStore,
    KyberPreKeyId, KyberPreKeyRecord, KyberPreKeyStore, PreKeyId, PreKeyRecord, PreKeyStore,
    ProtocolAddress, PublicKey, SessionRecord, SessionStore, SignalProtocolError, SignedPreKeyId,
    SignedPreKeyRecord, SignedPreKeyStore,
};
use redb::{ReadableDatabase, ReadableTable, TableDefinition};
use sha2::{Digest, Sha256};
use std::{array::TryFromSliceError, path::Path, sync::Arc, time::SystemTime};
use tokio::sync::Mutex;
use zeroize::Zeroizing;

pub mod file;

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
        limit: usize,
    ) -> Result<Vec<Vec<u8>>> {
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
            .range(start..=end)
            .map_err(|err| KursalError::Storage(err.to_string()))?
            .rev()
            .take(limit)
            .map(|entry| {
                let (_, v) = entry.map_err(|err| KursalError::Storage(err.to_string()))?;

                stream_decrypt(&self.key, v.value())
            })
            .collect()
    }

    pub(crate) fn raw_readall(&self, table: TableDefinition<&str, &[u8]>) -> Result<Vec<Vec<u8>>> {
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
                let (_, v) = entry.map_err(|err| KursalError::Storage(err.to_string()))?;

                let bytes = v.value();
                let nonce: [u8; 12] = bytes[0..12]
                    .try_into()
                    .map_err(|e: TryFromSliceError| KursalError::Crypto(e.to_string()))?;

                let cipher = Aes256Gcm::new_from_slice(&*self.key)
                    .map_err(|err| KursalError::Crypto(err.to_string()))?;

                cipher
                    .decrypt(&nonce.into(), &bytes[12..])
                    .map_err(|err| KursalError::Crypto(err.to_string()))
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

pub fn get_dilithium_pub(db: &Database) -> Result<Vec<u8>> {
    db.raw_read(TABLE_SETTINGS, "dilithium_public")?
        .ok_or(KursalError::Storage("No dilithium public key".to_string()))
}

pub fn relay_server_enabled(db: &Database) -> Result<bool> {
    let a = db
        .raw_read(TABLE_SETTINGS, "relay_server_enabled")?
        .unwrap_or(vec![0u8]);

    Ok(a == vec![1u8])
}

pub fn set_relay_server_enabled(db: &Database, value: bool) -> Result<()> {
    db.raw_write(
        TABLE_SETTINGS,
        "relay_server_enabled",
        if value { &[1u8] } else { &[0u8] },
    )?;

    Ok(())
}

pub fn get_local_profile(db: &Database) -> Result<(String, Option<Vec<u8>>)> {
    let username = std::str::from_utf8(
        &db.raw_read(TABLE_SETTINGS, "local_profile_username")?
            .unwrap_or("You".as_bytes().to_vec()),
    )
    .unwrap_or("You")
    .to_string();

    let avatar = db
        .raw_read(TABLE_SETTINGS, "local_profile_avatar")?
        .and_then(|bytes| if bytes.is_empty() { None } else { Some(bytes) });

    Ok((username, avatar))
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
