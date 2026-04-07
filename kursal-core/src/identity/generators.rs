use crate::{
    KursalError, Result,
    storage::{
        Database, SharedDatabase, TABLE_IDENTITY_KEYS, TABLE_SETTINGS, get_timestamp_millis,
    },
};
use libsignal_protocol::{
    GenericSignedPreKey, IdentityKeyPair, KeyPair, KyberPreKeyId, KyberPreKeyRecord,
    KyberPreKeyStore, PreKeyId, PreKeyRecord, PreKeyStore, SignedPreKeyId, SignedPreKeyRecord,
    SignedPreKeyStore, Timestamp, kem,
};
use pqcrypto_dilithium::dilithium5;
use pqcrypto_traits::sign::{PublicKey, SecretKey};
use rand::{TryRngCore, rngs::OsRng};
use zeroize::Zeroizing;

pub fn generate_identity_keypair(db: &mut Database) -> Result<IdentityKeyPair> {
    let mut rng = OsRng.unwrap_err();
    let idkey = IdentityKeyPair::generate(&mut rng);

    let serialized = idkey.serialize();
    db.raw_write(TABLE_IDENTITY_KEYS, "local_identity", &serialized)?;

    let regid = rand::random::<u32>();
    db.raw_write(TABLE_SETTINGS, "registration_id", &regid.to_be_bytes())?;

    Ok(idkey)
}

pub async fn generate_pre_key(mut db: SharedDatabase) -> Result<()> {
    let mut rng = OsRng.unwrap_err();
    let keypair = KeyPair::generate(&mut rng);

    let prekeyid = PreKeyId::from(1u32);
    let record = PreKeyRecord::new(prekeyid, &keypair);

    db.save_pre_key(prekeyid, &record)
        .await
        .map_err(|err| KursalError::Crypto(err.to_string()))?;

    Ok(())
}

pub async fn generate_signed_prekey(
    mut db: SharedDatabase,
    identity: &IdentityKeyPair,
) -> Result<()> {
    let mut rng = OsRng.unwrap_err();
    let key_pair = KeyPair::generate(&mut rng);

    let sig = identity
        .private_key()
        .calculate_signature(&key_pair.public_key.serialize(), &mut rng)
        .map_err(|err| KursalError::Crypto(err.to_string()))?;

    let timestamp = get_timestamp_millis()?;
    let signed_prekey_id = SignedPreKeyId::from(1u32);

    let signed_prekey = SignedPreKeyRecord::new(
        signed_prekey_id,
        Timestamp::from_epoch_millis(timestamp),
        &key_pair,
        &sig,
    );

    db.save_signed_pre_key(signed_prekey_id, &signed_prekey)
        .await
        .map_err(|err| KursalError::Crypto(err.to_string()))?;

    Ok(())
}

pub async fn generate_kyber_prekey(
    mut db: SharedDatabase,
    identity: &IdentityKeyPair,
) -> Result<()> {
    let mut rng = OsRng.unwrap_err();
    let key_pair = kem::KeyPair::generate(kem::KeyType::Kyber1024, &mut rng);

    let sig = identity
        .private_key()
        .calculate_signature(&key_pair.public_key.serialize(), &mut rng)
        .map_err(|err| KursalError::Crypto(err.to_string()))?;

    let timestamp = get_timestamp_millis()?;

    let kyber_prekey = KyberPreKeyRecord::new(
        1u32.into(), // TODO: maybe add this as parameter later
        Timestamp::from_epoch_millis(timestamp),
        &key_pair,
        &sig,
    );

    db.save_kyber_pre_key(KyberPreKeyId::from(1u32), &kyber_prekey)
        .await
        .map_err(|err| KursalError::Crypto(err.to_string()))?;

    Ok(())
}

pub fn generate_dilithium_keypair(db: &mut Database) -> Result<()> {
    let key_pair = dilithium5::keypair();

    let secret = Zeroizing::new(key_pair.1.as_bytes().to_vec());
    db.raw_write(TABLE_SETTINGS, "dilithium_secret", &secret)?;

    db.raw_write(TABLE_SETTINGS, "dilithium_public", key_pair.0.as_bytes())?;

    Ok(())
}
