use crate::{
    crypto::{
        PreKeyBundleData,
        messages::{message_receive, message_send},
        session_initiate,
    },
    first_contact::ltc::LtcPayload,
    identity::generators::{
        generate_dilithium_keypair, generate_identity_keypair, generate_kyber_prekey,
        generate_pre_key, generate_signed_prekey,
    },
    storage::{Database, SharedDatabase, get_dilithium_pub, get_timestamp_secs},
    tests::CACHE_DIR,
};
use libsignal_protocol::{DeviceId, ProtocolAddress};

async fn make_peer(name: &str) -> SharedDatabase {
    let path = CACHE_DIR.join(format!("ltc_test_{}.db", name));
    std::fs::create_dir_all(&*CACHE_DIR).unwrap();
    let _ = std::fs::remove_file(&path);

    let mut db = Database::open(&path, [0u8; 32]).unwrap();
    let identity = generate_identity_keypair(&mut db).unwrap();
    generate_dilithium_keypair(&mut db).unwrap();

    let sdb = SharedDatabase::from_db(db);
    generate_signed_prekey(sdb.clone(), &identity)
        .await
        .unwrap();
    generate_kyber_prekey(sdb.clone(), &identity).await.unwrap();
    generate_pre_key(sdb.clone()).await.unwrap();

    sdb
}

// Alice generates LTC, Bob deserializes and initiates session,
// Alice initiates session back, Alice encrypts → Bob decrypts
#[tokio::test]
async fn ltc_full_session_roundtrip() {
    let alice = make_peer("ltc_alice").await;
    let bob = make_peer("ltc_bob").await;

    let alice_address = ProtocolAddress::new("alice".to_string(), DeviceId::new(1u8).unwrap());
    let bob_address = ProtocolAddress::new("bob".to_string(), DeviceId::new(1u8).unwrap());

    // Alice builds LTC bundle (no one-time prekey)
    let alice_bundle = PreKeyBundleData::build_pre_key_bundle_noprekey(alice.clone())
        .await
        .unwrap();
    let now = get_timestamp_secs().unwrap();

    let ltc = LtcPayload {
        peer_id: "alice".to_string(),
        pre_key_bundle: alice_bundle.serialize().unwrap(),
        dilithium_pub_key: get_dilithium_pub(&*alice.0.lock().await).unwrap(),
        relay_addresses: vec![],
        created_at: now,
        expires_at: now + 604800,
    };

    // simulate file transfer
    let ltc_bytes = ltc.serialize().unwrap();
    let ltc_received = LtcPayload::deserialize(&ltc_bytes).unwrap();

    // Bob initiates session with Alice
    let alice_bundle_for_bob = PreKeyBundleData::deserialize(&ltc_received.pre_key_bundle).unwrap();
    session_initiate(bob.clone(), alice_bundle_for_bob, &alice_address)
        .await
        .unwrap();

    // Alice initiates session with Bob
    let bob_bundle = PreKeyBundleData::build_pre_key_bundle(bob.clone())
        .await
        .unwrap();
    session_initiate(alice.clone(), bob_bundle, &bob_address)
        .await
        .unwrap();

    // Alice encrypts → Bob decrypts
    let plaintext = b"hello from alice via ltc";
    let ciphertext = message_send(alice.clone(), &bob_address, plaintext)
        .await
        .unwrap();
    let decrypted = message_receive(bob.clone(), &alice_address, &ciphertext)
        .await
        .unwrap();

    assert_eq!(decrypted, plaintext);
}

// Expired LTC is detected correctly
#[test]
fn ltc_is_expired() {
    let now = get_timestamp_secs().unwrap();
    let ltc = LtcPayload {
        peer_id: "alice".to_string(),
        pre_key_bundle: vec![],
        dilithium_pub_key: vec![],
        relay_addresses: vec![],
        created_at: now - 604900,
        expires_at: now - 100,
    };
    assert!(ltc.is_expired());
}

// Valid LTC is not expired
#[test]
fn ltc_not_expired() {
    let now = get_timestamp_secs().unwrap();
    let ltc = LtcPayload {
        peer_id: "alice".to_string(),
        pre_key_bundle: vec![],
        dilithium_pub_key: vec![],
        relay_addresses: vec![],
        created_at: now,
        expires_at: now + 604800,
    };
    assert!(!ltc.is_expired());
}
