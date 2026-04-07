use crate::{
    storage::{Database, SharedDatabase, TABLE_IDENTITY_KEYS, TABLE_MESSAGES},
    tests::CACHE_DIR,
};
use libsignal_protocol::{
    DeviceId, IdentityKeyPair, IdentityKeyStore, ProtocolAddress, SessionRecord, SessionStore,
};
use rand::{TryRngCore, rngs::OsRng};

fn setup_db(name: &str, key: [u8; 32]) -> Database {
    std::fs::create_dir_all(&*CACHE_DIR).unwrap();
    Database::open(&CACHE_DIR.join(name), key).unwrap()
}

// raw_write then raw_read → assert plaintext matches
#[test]
fn storage_write_read() {
    let key = [1u8; 32];
    let db = setup_db("storage_write_read.db", key);

    let written = [1u8, 2u8, 3u8];
    db.raw_write(TABLE_MESSAGES, "test_key", &written).unwrap();

    let read = db.raw_read(TABLE_MESSAGES, "test_key").unwrap().unwrap();
    assert_eq!(read, written);
}

// raw_read with wrong enc_key → assert decrypt error
#[test]
fn storage_wrong_key() {
    let key = [1u8; 32];
    let wrong_key = [2u8; 32];

    {
        let db = setup_db("storage_wrong_key.db", key);
        db.raw_write(TABLE_MESSAGES, "test_key", &[1u8, 2u8, 3u8])
            .unwrap();
    }

    let db_wrong = setup_db("storage_wrong_key.db", wrong_key);
    let result = db_wrong.raw_read(TABLE_MESSAGES, "test_key");
    assert!(result.is_err());
}

// raw_delete then raw_read → assert None
#[test]
fn storage_delete() {
    let key = [1u8; 32];
    let db = setup_db("storage_delete.db", key);

    db.raw_write(TABLE_MESSAGES, "test_key", &[1u8, 2u8, 3u8])
        .unwrap();
    db.raw_delete(TABLE_MESSAGES, "test_key").unwrap();

    let result = db.raw_read(TABLE_MESSAGES, "test_key").unwrap();
    assert_eq!(result, None);
}

// Store a SessionRecord, load it back, assert it deserializes correctly
#[tokio::test]
async fn storage_session_record_roundtrip() {
    let key = [3u8; 32];
    let mut db = SharedDatabase::from_db(setup_db("storage_session_record.db", key));

    let address = ProtocolAddress::new("alice".to_string(), DeviceId::new(1u8).unwrap());
    let record = SessionRecord::new_fresh();

    db.store_session(&address, &record).await.unwrap();

    let loaded = db.load_session(&address).await.unwrap().unwrap();
    assert_eq!(record.serialize().unwrap(), loaded.serialize().unwrap());
}

// Store an IdentityKeyPair, load it back via get_identity_key_pair, assert matches
#[tokio::test]
async fn storage_identity_key_pair_roundtrip() {
    let key = [4u8; 32];
    let db = setup_db("storage_identity_key_pair.db", key);

    let mut rng = OsRng.unwrap_err();
    let identity_key_pair = IdentityKeyPair::generate(&mut rng);
    let serialized = identity_key_pair.serialize();

    db.raw_write(TABLE_IDENTITY_KEYS, "local_identity", &serialized)
        .unwrap();

    let sdb = SharedDatabase::from_db(db);

    let loaded = sdb.get_identity_key_pair().await.unwrap();
    assert_eq!(identity_key_pair.serialize(), loaded.serialize());
}
