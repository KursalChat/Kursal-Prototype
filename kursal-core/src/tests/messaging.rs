use crate::{
    crypto::{
        PreKeyBundleData,
        dilithium::{dilithium_sign, dilithium_verify},
        messages::{message_receive, message_send},
        session_initiate,
    },
    identity::{
        self, UserId,
        keychain::{self, KeychainConfig},
    },
    messaging::{
        StoredMessage,
        enums::{Direction, KursalMessage, MessageId, MessageStatus, TextMessage},
    },
    storage::{Database, SharedDatabase, TABLE_SETTINGS, get_dilithium_pub, get_timestamp_secs},
    tests::{APP_DATA_DIR, CACHE_DIR},
};
use libsignal_protocol::{DeviceId, ProtocolAddress};

// ──────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────

/// Open a fully-initialised database (all keys generated).
async fn make_db(name: &str) -> SharedDatabase {
    let path = CACHE_DIR.join(format!("msg_test_{name}.db"));
    std::fs::create_dir_all(&*CACHE_DIR).unwrap();
    let _ = std::fs::remove_file(&path);
    identity::init(
        &path,
        &KeychainConfig {
            storage_id: "test".to_string(),
            unsafe_write_key_to_file: false,
        },
        &APP_DATA_DIR,
    )
    .await
    .unwrap()
}

/// Establish Signal sessions on both sides so both can send and receive.
///
/// `alice_addr` is the address Alice registers herself under (how Bob addresses her).
/// `bob_addr`   is the address Bob   registers himself under (how Alice addresses him).
async fn setup_sessions(
    alice: SharedDatabase,
    bob: SharedDatabase,
    alice_addr: &ProtocolAddress,
    bob_addr: &ProtocolAddress,
) {
    let alice_bundle = PreKeyBundleData::build_pre_key_bundle(alice.clone())
        .await
        .unwrap();
    let bob_bundle = PreKeyBundleData::build_pre_key_bundle(bob.clone())
        .await
        .unwrap();

    // Bob ingests Alice's bundle → can now send to alice_addr
    session_initiate(bob.clone(), alice_bundle, alice_addr)
        .await
        .unwrap();
    // Alice ingests Bob's bundle → can now send to bob_addr
    session_initiate(alice.clone(), bob_bundle, bob_addr)
        .await
        .unwrap();
}

// ──────────────────────────────────────────────────────────────
// Test 1: full round-trip with delivery receipt
// ──────────────────────────────────────────────────────────────

/// Alice sends a TextMessage.
/// Bob decrypts it and signs a delivery receipt over the raw ciphertext.
/// Alice verifies the receipt and marks the message as Delivered.
#[tokio::test]
async fn test_message_roundtrip_with_receipt() {
    keychain::init_keychain().unwrap();
    let alice = make_db("msg_rt_alice").await;
    let bob = make_db("msg_rt_bob").await;

    // Use fixed hex strings as protocol addresses for reproducibility.
    let alice_addr = ProtocolAddress::new("alice_rt".to_string(), DeviceId::new(1u8).unwrap());
    let bob_addr = ProtocolAddress::new("bob_rt".to_string(), DeviceId::new(1u8).unwrap());

    setup_sessions(alice.clone(), bob.clone(), &alice_addr, &bob_addr).await;

    // ── Alice composes and encrypts a message ──────────────────
    let msg_id = MessageId::new();
    let outgoing = KursalMessage::Text(TextMessage {
        id: msg_id.clone(),
        content: "hello bob".to_string(),
        timestamp: get_timestamp_secs().unwrap(),
        reply_to: None,
    });
    let plaintext = outgoing.serialize().unwrap();
    let ciphertext = message_send(alice.clone(), &bob_addr, &plaintext)
        .await
        .unwrap();

    // Alice stores the message as Sent/Sending with the raw ciphertext saved.
    // raw_ciphertext is what will be signed by Bob for the receipt.
    let alice_view_of_bob = UserId([0xBBu8; 32]); // placeholder contact id
    let sent = StoredMessage {
        id: msg_id.clone(),
        contact_id: alice_view_of_bob.clone(),
        direction: Direction::Sent,
        payload: outgoing,
        status: MessageStatus::Sending,
        timestamp: get_timestamp_secs().unwrap(),
        raw_ciphertext: Some(ciphertext.clone()),
        edited: false,
        reactions: vec![],
    };
    sent.save(&*alice.0.lock().await).unwrap();

    // ── Bob decrypts ──────────────────────────────────────────
    let decrypted = message_receive(bob.clone(), &alice_addr, &ciphertext)
        .await
        .unwrap();
    let received_msg = KursalMessage::deserialize(&decrypted).unwrap();

    let KursalMessage::Text(ref text) = received_msg else {
        panic!("Expected KursalMessage::Text, got something else");
    };
    assert_eq!(text.content, "hello bob");

    // ── Bob signs a delivery receipt over the raw ciphertext ──
    let bob_dil_secret = bob
        .0
        .lock()
        .await
        .raw_read(TABLE_SETTINGS, "dilithium_secret")
        .unwrap()
        .expect("dilithium_secret must exist after identity::init");

    let receipt_sig = dilithium_sign(&bob_dil_secret, &ciphertext).unwrap();

    // ── Alice verifies the receipt ────────────────────────────
    // In real code, bob_dil_pub comes from Contact::dilithium_pub_key.
    // In this test we load it from Bob's DB directly — same bytes.
    let bob_dil_pub = get_dilithium_pub(&*bob.0.lock().await).unwrap();

    let valid = dilithium_verify(&bob_dil_pub, &ciphertext, &receipt_sig).unwrap();
    assert!(valid, "Receipt signature should be valid");

    // ── Alice updates status to Delivered ─────────────────────
    let mut stored = StoredMessage::load(&*alice.0.lock().await, &alice_view_of_bob, &msg_id)
        .unwrap()
        .expect("message must exist in Alice's DB");

    stored.status = MessageStatus::Delivered;
    stored.save(&*alice.0.lock().await).unwrap();

    // Confirm the persisted status is Delivered
    let final_msg = StoredMessage::load(&*alice.0.lock().await, &alice_view_of_bob, &msg_id)
        .unwrap()
        .unwrap();

    assert!(
        matches!(final_msg.status, MessageStatus::Delivered),
        "Message should be Delivered after receipt is verified"
    );
}

// ──────────────────────────────────────────────────────────────
// Test 2: tampered receipt must not advance the status
// ──────────────────────────────────────────────────────────────

/// Bob signs the WRONG bytes (simulating a tampered or forged receipt).
/// `dilithium_verify` must return false.
/// Alice must not update the message status — it stays Sending.
#[tokio::test]
async fn test_tampered_receipt_stays_sending() {
    keychain::init_keychain().unwrap();
    let alice = make_db("msg_tamper_alice").await;
    let bob = make_db("msg_tamper_bob").await;

    let alice_addr = ProtocolAddress::new("alice_tamper".to_string(), DeviceId::new(1u8).unwrap());
    let bob_addr = ProtocolAddress::new("bob_tamper".to_string(), DeviceId::new(1u8).unwrap());

    setup_sessions(alice.clone(), bob.clone(), &alice_addr, &bob_addr).await;

    // Alice encrypts and stores a message
    let msg_id = MessageId::new();
    let content = KursalMessage::Text(TextMessage {
        id: msg_id.clone(),
        content: "tamper test".to_string(),
        timestamp: get_timestamp_secs().unwrap(),
        reply_to: None,
    });
    let plaintext = content.serialize().unwrap();
    let ciphertext = message_send(alice.clone(), &bob_addr, &plaintext)
        .await
        .unwrap();

    let contact_id = UserId([0xBBu8; 32]);
    let sent = StoredMessage {
        id: msg_id.clone(),
        contact_id: contact_id.clone(),
        direction: Direction::Sent,
        payload: content,
        status: MessageStatus::Sending,
        timestamp: get_timestamp_secs().unwrap(),
        raw_ciphertext: Some(ciphertext.clone()),
        edited: false,
        reactions: vec![],
    };
    sent.save(&*alice.0.lock().await).unwrap();

    // Bob signs DIFFERENT bytes — one byte flipped relative to the real ciphertext
    let bob_dil_secret = bob
        .0
        .lock()
        .await
        .raw_read(TABLE_SETTINGS, "dilithium_secret")
        .unwrap()
        .unwrap();

    let mut wrong_bytes = ciphertext.clone();
    wrong_bytes[0] ^= 0xff;
    let bad_sig = dilithium_sign(&bob_dil_secret, &wrong_bytes).unwrap();

    let bob_dil_pub = get_dilithium_pub(&*bob.0.lock().await).unwrap();

    // Verifying against the ORIGINAL ciphertext with a sig over different bytes → false
    let valid = dilithium_verify(&bob_dil_pub, &ciphertext, &bad_sig).unwrap();
    assert!(
        !valid,
        "A receipt signed over different bytes must not verify"
    );

    // Alice does NOT update status (because !valid) — status must remain Sending
    // This mirrors the behaviour in handle_incoming: only update when signature is valid.
    let stored = StoredMessage::load(&*alice.0.lock().await, &contact_id, &msg_id)
        .unwrap()
        .unwrap();

    assert!(
        matches!(stored.status, MessageStatus::Sending),
        "Status must remain Sending when the receipt signature is invalid"
    );
}

// ──────────────────────────────────────────────────────────────
// Test 3: load_all returns only messages for the requested contact
// ──────────────────────────────────────────────────────────────

/// Stores messages for three different contacts (2 / 3 / 1).
/// Asserts that load_all returns exactly the right count for each,
/// with no cross-contamination between contacts.
#[tokio::test]
async fn test_load_all_filters_by_contact() {
    // Plain Database is sufficient here — no crypto needed.
    let path = CACHE_DIR.join("msg_test_load_all_filter.db");
    std::fs::create_dir_all(&*CACHE_DIR).unwrap();
    let _ = std::fs::remove_file(&path);
    let db = Database::open(&path, [0u8; 32]).unwrap();

    let alice_id = UserId([0xAAu8; 32]);
    let bob_id = UserId([0xBBu8; 32]);
    let charlie_id = UserId([0xCCu8; 32]);

    let make_msg = |contact: UserId, text: &str| {
        let id = MessageId::new();
        StoredMessage {
            id: id.clone(),
            contact_id: contact,
            direction: Direction::Sent,
            payload: KursalMessage::Text(TextMessage {
                id: id,
                content: text.to_string(),
                timestamp: get_timestamp_secs().unwrap(),
                reply_to: None,
            }),
            status: MessageStatus::Delivered,
            timestamp: get_timestamp_secs().unwrap(),
            raw_ciphertext: None,
            edited: false,
            reactions: vec![],
        }
    };

    // 2 messages for Alice
    for i in 0..2u8 {
        make_msg(alice_id.clone(), &format!("alice {i}"))
            .save(&db)
            .unwrap();
    }
    // 3 messages for Bob
    for i in 0..3u8 {
        make_msg(bob_id.clone(), &format!("bob {i}"))
            .save(&db)
            .unwrap();
    }
    // 1 message for Charlie
    make_msg(charlie_id.clone(), "charlie 0").save(&db).unwrap();

    let alice_msgs = StoredMessage::load_all(&db, &alice_id, 100, None).unwrap();
    let bob_msgs = StoredMessage::load_all(&db, &bob_id, 100, None).unwrap();
    let charlie_msgs = StoredMessage::load_all(&db, &charlie_id, 100, None).unwrap();

    assert_eq!(alice_msgs.len(), 2, "Expected 2 messages for alice");
    assert_eq!(bob_msgs.len(), 3, "Expected 3 messages for bob");
    assert_eq!(charlie_msgs.len(), 1, "Expected 1 message for charlie");

    // No cross-contamination
    assert!(
        alice_msgs.iter().all(|m| m.contact_id.0 == alice_id.0),
        "Alice's messages contain wrong contact"
    );
    assert!(
        bob_msgs.iter().all(|m| m.contact_id.0 == bob_id.0),
        "Bob's messages contain wrong contact"
    );
    assert!(
        charlie_msgs.iter().all(|m| m.contact_id.0 == charlie_id.0),
        "Charlie's messages contain wrong contact"
    );
}
