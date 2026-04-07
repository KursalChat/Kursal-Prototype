use crate::{
    KursalError, Result,
    identity::{
        generators::{
            generate_dilithium_keypair, generate_identity_keypair, generate_kyber_prekey,
            generate_pre_key, generate_signed_prekey,
        },
        keychain::{
            KeychainConfig, generate_master_secret, get_entry, load_master_secret,
            store_master_secret,
        },
    },
    storage::{Database, SharedDatabase, TABLE_IDENTITY_KEYS, TABLE_SETTINGS, derive_db_key},
};
use libp2p::PeerId;
use libsignal_protocol::IdentityKeyStore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

pub mod generators;
pub mod keychain;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct UserId(pub [u8; 32]);
impl UserId {
    pub async fn local_user_id(db: SharedDatabase) -> Result<UserId> {
        let keypair = db
            .get_identity_key_pair()
            .await
            .map_err(|err| KursalError::Crypto(err.to_string()))?;

        let serialized = keypair.public_key().serialize();
        let hash: [u8; 32] = Sha256::digest(&serialized).into();

        Ok(UserId(hash))
    }
}

// TODO: if generate_identity_keypair works but another fails then it's weird state.. help
pub async fn init(
    db_path: &Path,
    keychain_config: &KeychainConfig,
    app_data_dir: &Path,
) -> Result<SharedDatabase> {
    let entry = get_entry(keychain_config)?;

    let master_key =
        if let Some(master_key) = load_master_secret(keychain_config, &entry, app_data_dir)? {
            master_key
        } else {
            let generated = generate_master_secret()?;
            store_master_secret(&generated, keychain_config, &entry, app_data_dir)?;

            generated.to_vec()
        };

    let db_enc = derive_db_key(&master_key)?;
    let mut db = Database::open(db_path, db_enc)?;

    if db
        .raw_read(TABLE_IDENTITY_KEYS, "local_identity")?
        .is_some()
    {
        let shared_db = SharedDatabase::from_db(db);
        return Ok(shared_db);
    }

    let identity = generate_identity_keypair(&mut db)?;
    generate_dilithium_keypair(&mut db)?;

    let shared_db = SharedDatabase::from_db(db);
    generate_signed_prekey(shared_db.clone(), &identity).await?;
    generate_kyber_prekey(shared_db.clone(), &identity).await?;
    generate_pre_key(shared_db.clone()).await?;

    Ok(shared_db)
}

pub fn init_transport(db: &Database) -> Result<TransportIdentity> {
    match TransportIdentity::load(db)? {
        Some(id) => Ok(id),
        None => {
            let id = TransportIdentity::generate();
            id.save(db)?;
            Ok(id)
        }
    }
}

pub struct TransportIdentity {
    pub keypair: libp2p::identity::Keypair,
    pub peer_id: PeerId,
}

impl TransportIdentity {
    pub fn generate() -> Self {
        let keypair = libp2p::identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from_public_key(&keypair.public());

        Self { keypair, peer_id }
    }

    fn save_under(&self, db: &Database, key: &str) -> Result<()> {
        let serialized = self
            .keypair
            .to_protobuf_encoding()
            .map_err(|err| KursalError::Crypto(err.to_string()))?;

        db.raw_write(TABLE_SETTINGS, key, &serialized)?;

        Ok(())
    }

    pub fn save(&self, db: &Database) -> Result<()> {
        self.save_under(db, "transport_identity")
    }

    pub fn save_next(&self, db: &Database) -> Result<()> {
        self.save_under(db, "transport_identity_next")
    }

    fn load_under(db: &Database, key: &str) -> Result<Option<Self>> {
        let fetched = db.raw_read(TABLE_SETTINGS, key)?;

        match fetched {
            Some(b) => {
                let keypair = libp2p::identity::Keypair::from_protobuf_encoding(&b)
                    .map_err(|err| KursalError::Crypto(err.to_string()))?;
                let peer_id = PeerId::from_public_key(&keypair.public());

                Ok(Some(Self { keypair, peer_id }))
            }
            None => Ok(None),
        }
    }

    pub fn load(db: &Database) -> Result<Option<Self>> {
        TransportIdentity::load_under(db, "transport_identity")
    }

    pub fn load_next(db: &Database) -> Result<Option<Self>> {
        TransportIdentity::load_under(db, "transport_identity_next")
    }
}

pub fn security_code(
    local_identity_pub: &[u8],
    local_dilithium_pub: &[u8],
    remote_identity_pub: &[u8],
    remote_dilithium_pub: &[u8],
) -> String {
    let mut id = [local_identity_pub, remote_identity_pub];
    id.sort();
    let id = id.concat();

    let mut dili = [local_dilithium_pub, remote_dilithium_pub];
    dili.sort();
    let dili = dili.concat();

    let both = [id, dili].concat();
    let hash: [u8; 32] = Sha256::digest(both).into();

    (0..8)
        .map(|i| {
            let chunk = u32::from_be_bytes(hash[i * 4..i * 4 + 4].try_into().unwrap());
            format!("{:04}", chunk % 10000)
        })
        .collect::<Vec<_>>()
        .join(" ")
}
