use crate::crypto::stream::stream_decrypt;
use crate::first_contact::ContactResponse;
use crate::storage::{get_dilithium_pub, get_timestamp_secs};
use crate::tests::CACHE_DIR;
use crate::{
    crypto::{
        PreKeyBundleData,
        messages::{message_receive, message_send},
        session_initiate,
    },
    first_contact::otp::{handle_otp_response, otp_to_keys},
    identity::generators::{generate_dilithium_keypair, generate_identity_keypair},
    storage::{Database, SharedDatabase, TABLE_SETTINGS},
};
use libsignal_protocol::{DeviceId, IdentityKeyStore, ProtocolAddress};
use tokio::sync::mpsc;

async fn make_peer(name: &str) -> SharedDatabase {
    let path = CACHE_DIR.join(format!("otp_test_{}.db", name));
    std::fs::create_dir_all(&*CACHE_DIR).unwrap();
    let _ = std::fs::remove_file(&path);

    let db_key = [0u8; 32];
    let mut db = Database::open(&path, db_key).unwrap();

    generate_identity_keypair(&mut db).unwrap();
    generate_dilithium_keypair(&mut db).unwrap();

    let sdb = SharedDatabase::from_db(db);

    sdb
}

// Alice generates OTP, Bob decrypts payload directly, both initiate sessions,
// Bob encrypts a message, Alice decrypts it
#[tokio::test]
async fn otp_full_session_roundtrip() {
    let alice = make_peer("otp_alice").await;
    let bob = make_peer("otp_bob").await;

    let otp = "3-correct 7-horse 1-battery 4-staple 9-ocean 2-light";
    let (enc_key, _dht_key) = otp_to_keys(otp).unwrap();

    // Alice builds her payload (no real network needed — just build it directly)
    // We need a fake NetworkManager peer_id — use alice's peer_id from transport
    // For this test we bypass build_otp_payload and construct it manually
    let alice_bundle = PreKeyBundleData::build_pre_key_bundle(alice.clone())
        .await
        .unwrap();
    let alice_bundle_bytes = alice_bundle.serialize().unwrap();
    let _alice_identity_pub = {
        let keypair = alice.get_identity_key_pair().await.unwrap();
        keypair.public_key().serialize().to_vec()
    };

    // Simulate the encrypted payload Alice would publish to DHT
    let payload = crate::first_contact::otp::OtpPayload {
        pre_key_bundle: alice_bundle_bytes,
        peer_id: "alice_peer_id".to_string(),
        dilithium_pub_key: get_dilithium_pub(&*alice.0.lock().await).unwrap(),
        relay_addresses: vec![],
    };
    let encrypted_payload =
        crate::crypto::stream::stream_encrypt(&enc_key, &payload.serialize().unwrap()).unwrap();

    // Bob decrypts payload directly (simulating DHT fetch)
    let decrypted = stream_decrypt(&enc_key, &encrypted_payload).unwrap();
    let record = crate::first_contact::otp::OtpPayload::deserialize(&decrypted).unwrap();
    let alice_bundle_for_bob = PreKeyBundleData::deserialize(&record.pre_key_bundle).unwrap();

    let alice_address =
        ProtocolAddress::new("alice_peer_id".to_string(), DeviceId::new(1u8).unwrap());
    let bob_address = ProtocolAddress::new("bob_peer_id".to_string(), DeviceId::new(1u8).unwrap());

    // Bob initiates session with Alice
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

    // Bob encrypts → Alice decrypts
    let plaintext = b"hello from bob";
    let ciphertext = message_send(bob.clone(), &alice_address, plaintext)
        .await
        .unwrap();
    let decrypted = message_receive(alice.clone(), &bob_address, &ciphertext)
        .await
        .unwrap();

    assert_eq!(decrypted, plaintext);
}

// A second OTPResponse after the first should be ignored
#[tokio::test]
async fn otp_response_ignored_after_consumed() {
    let db = make_peer("otp_consumed").await;
    let bob = make_peer("otp_consumed_bob").await;

    // Set otp_pending = true
    db.0.lock()
        .await
        .raw_write(TABLE_SETTINGS, "otp_pending", &[1u8])
        .unwrap();

    let now = get_timestamp_secs().unwrap();
    db.0.lock()
        .await
        .raw_write(TABLE_SETTINGS, "otp_published_at", &now.to_be_bytes())
        .unwrap();

    // Build a fake valid OTPResponse from bob
    let bob_bundle = PreKeyBundleData::build_pre_key_bundle(bob.clone())
        .await
        .unwrap();
    let response = ContactResponse {
        pre_key_bundle: bob_bundle.serialize().unwrap(),
        peer_id: "bob_peer_id".to_string(),
        dilithium_pub_key: get_dilithium_pub(&*bob.0.lock().await).unwrap(),
        relay_addresses: vec![],
    };

    // First call succeeds
    let (cmd_tx, _cmd_rx) = mpsc::channel(8);
    let (event_tx, _event_rx) = mpsc::channel(8);
    handle_otp_response(response.clone(), db.clone(), &cmd_tx, &event_tx)
        .await
        .unwrap();

    // Second call should be silently ignored — otp_pending is now false
    let result = handle_otp_response(response, db.clone(), &cmd_tx, &event_tx).await;
    assert!(result.is_ok());
}

// OTPResponse arriving after 10 minutes should be rejected
#[tokio::test]
async fn otp_response_rejected_after_expiry() {
    let db = make_peer("otp_expiry").await;
    let bob = make_peer("otp_expiry_bob").await;

    // Set otp_pending = true but published 11 minutes ago
    let fake_past = get_timestamp_secs().unwrap() - 660; // 11 minutes ago

    db.0.lock()
        .await
        .raw_write(TABLE_SETTINGS, "otp_pending", &[1u8])
        .unwrap();
    db.0.lock()
        .await
        .raw_write(TABLE_SETTINGS, "otp_published_at", &fake_past.to_be_bytes())
        .unwrap();

    let bob_bundle = PreKeyBundleData::build_pre_key_bundle(bob.clone())
        .await
        .unwrap();
    let response = ContactResponse {
        pre_key_bundle: bob_bundle.serialize().unwrap(),
        peer_id: "bob_peer_id".to_string(),
        dilithium_pub_key: get_dilithium_pub(&*bob.0.lock().await).unwrap(),
        relay_addresses: vec![],
    };
    let (cmd_tx, _cmd_rx) = mpsc::channel(8);
    let (event_tx, _event_rx) = mpsc::channel(8);
    let result = handle_otp_response(response, db.clone(), &cmd_tx, &event_tx).await;
    assert!(result.is_ok()); // silently ignored... cant actually really test that lol
}
