//! - PQXDH is done via `AliceSignalProtocolParameters` + `initialize_alice_session_record` and `BobSignalProtocolParameters` + `initialize_bob_session_record`.
//!
//! - Double Ratchet done via `message_encrypt` / `message_decrypt` with `&mut Database` as store.
//! - `PreKeyBundle` + `process_prekey_bundle` for the standard prekey flow.

use crate::{KursalError, Result, storage::SharedDatabase};
use libsignal_protocol::{
    DeviceId, GenericSignedPreKey, IdentityKey, IdentityKeyStore, KyberPreKeyId, KyberPreKeyStore,
    PreKeyBundle, PreKeyId, PreKeyStore, ProtocolAddress, PublicKey as SignalPublicKey,
    SignedPreKeyId, SignedPreKeyStore, kem, process_prekey_bundle,
};
use rand::{TryRngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

pub mod dilithium;
pub mod messages;
pub mod stream;

pub struct PreKeyBundleData {
    pub registration_id: u32,
    // pub device_id: u32,
    pub pre_key_id: Option<PreKeyId>,
    pub pre_key_public: Option<SignalPublicKey>,
    pub signed_pre_key_id: SignedPreKeyId,
    pub signed_pre_key_public: SignalPublicKey,
    pub signed_pre_key_signature: Vec<u8>,
    pub identity_key: IdentityKey,
    pub kyber_pre_key_id: KyberPreKeyId,
    pub kyber_pre_key_public: kem::PublicKey,
    pub kyber_pre_key_signature: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub struct PreKeyBundleBytes {
    pub registration_id: u32,
    pub pre_key_id: Option<u32>,
    pub pre_key_public: Option<Vec<u8>>,
    pub signed_pre_key_id: u32,
    pub signed_pre_key_public: Vec<u8>,
    pub signed_pre_key_signature: Vec<u8>,
    pub identity_key: Vec<u8>,
    pub kyber_pre_key_id: u32,
    pub kyber_pre_key_public: Vec<u8>,
    pub kyber_pre_key_signature: Vec<u8>,
}

impl PreKeyBundleData {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let bytes = PreKeyBundleBytes {
            registration_id: self.registration_id,
            pre_key_id: self.pre_key_id.map(|e| e.into()),
            pre_key_public: self.pre_key_public.as_ref().map(|k| k.serialize().to_vec()),
            signed_pre_key_id: self.signed_pre_key_id.into(),
            signed_pre_key_public: self.signed_pre_key_public.serialize().to_vec(),
            signed_pre_key_signature: self.signed_pre_key_signature.clone(),
            identity_key: self.identity_key.serialize().to_vec(),
            kyber_pre_key_id: self.kyber_pre_key_id.into(),
            kyber_pre_key_public: self.kyber_pre_key_public.serialize().to_vec(),
            kyber_pre_key_signature: self.kyber_pre_key_signature.clone(),
        };

        bincode::serialize(&bytes).map_err(|err| KursalError::Storage(err.to_string()))
    }

    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        let bundle = bincode::deserialize::<PreKeyBundleBytes>(bytes)
            .map_err(|err| KursalError::Storage(err.to_string()))?;

        Ok(PreKeyBundleData {
            registration_id: bundle.registration_id,
            pre_key_id: bundle.pre_key_id.map(|e| e.into()),
            pre_key_public: bundle
                .pre_key_public
                .as_deref()
                .map(|b| {
                    SignalPublicKey::deserialize(b)
                        .map_err(|err| KursalError::Crypto(err.to_string()))
                })
                .transpose()?,
            signed_pre_key_id: bundle.signed_pre_key_id.into(),
            signed_pre_key_public: SignalPublicKey::deserialize(&bundle.signed_pre_key_public)
                .map_err(|err| KursalError::Crypto(err.to_string()))?,
            signed_pre_key_signature: bundle.signed_pre_key_signature,
            identity_key: IdentityKey::decode(&bundle.identity_key)
                .map_err(|err| KursalError::Crypto(err.to_string()))?,
            kyber_pre_key_id: bundle.kyber_pre_key_id.into(),
            kyber_pre_key_public: kem::PublicKey::deserialize(&bundle.kyber_pre_key_public)
                .map_err(|err| KursalError::Crypto(err.to_string()))?,
            kyber_pre_key_signature: bundle.kyber_pre_key_signature,
        })
    }

    async fn builder_build_pre_key_bundle(
        db: SharedDatabase,
        with_prekey: bool,
    ) -> Result<PreKeyBundleData> {
        let registration_id = db
            .get_local_registration_id()
            .await
            .map_err(|err| KursalError::Storage(err.to_string()))?;

        let (prekey_id, prekey) = {
            if with_prekey {
                // TODO: this could just generate a new prekey instead
                let prekey_id = PreKeyId::from(1u32);

                (
                    Some(prekey_id),
                    Some(
                        db.get_pre_key(prekey_id)
                            .await
                            .map_err(|err| KursalError::Storage(err.to_string()))?
                            .public_key()
                            .map_err(|err| KursalError::Storage(err.to_string()))?,
                    ),
                )
            } else {
                (None, None)
            }
        };

        let signed_prekey_id = SignedPreKeyId::from(1u32);
        let signed_prekey = db
            .get_signed_pre_key(signed_prekey_id)
            .await
            .map_err(|err| KursalError::Storage(err.to_string()))?;

        let identity_keypair = db
            .get_identity_key_pair()
            .await
            .map_err(|err| KursalError::Storage(err.to_string()))?;

        let kyber_prekey_id = KyberPreKeyId::from(1u32);
        let kyber_prekey = db
            .get_kyber_pre_key(kyber_prekey_id)
            .await
            .map_err(|err| KursalError::Storage(err.to_string()))?;

        Ok(PreKeyBundleData {
            registration_id,
            pre_key_id: prekey_id,
            pre_key_public: prekey,
            signed_pre_key_id: signed_prekey_id,
            signed_pre_key_public: signed_prekey
                .public_key()
                .map_err(|err| KursalError::Storage(err.to_string()))?,
            signed_pre_key_signature: signed_prekey
                .signature()
                .map_err(|err| KursalError::Storage(err.to_string()))?,
            identity_key: *identity_keypair.identity_key(),
            kyber_pre_key_id: kyber_prekey_id,
            kyber_pre_key_public: kyber_prekey
                .public_key()
                .map_err(|err| KursalError::Storage(err.to_string()))?,
            kyber_pre_key_signature: kyber_prekey
                .signature()
                .map_err(|err| KursalError::Storage(err.to_string()))?,
        })
    }

    pub async fn build_pre_key_bundle(db: SharedDatabase) -> Result<PreKeyBundleData> {
        PreKeyBundleData::builder_build_pre_key_bundle(db, true).await
    }

    pub async fn build_pre_key_bundle_noprekey(db: SharedDatabase) -> Result<PreKeyBundleData> {
        PreKeyBundleData::builder_build_pre_key_bundle(db, false).await
    }
}

pub async fn session_initiate(
    db: SharedDatabase,
    remote: PreKeyBundleData,
    remote_address: &ProtocolAddress,
) -> Result<()> {
    let bundle = PreKeyBundle::new(
        remote.registration_id,
        DeviceId::new(1u8).unwrap(),
        remote.pre_key_id.zip(remote.pre_key_public),
        remote.signed_pre_key_id,
        remote.signed_pre_key_public,
        remote.signed_pre_key_signature,
        remote.kyber_pre_key_id,
        remote.kyber_pre_key_public,
        remote.kyber_pre_key_signature,
        remote.identity_key,
    )
    .map_err(|err| KursalError::Storage(err.to_string()))?;

    let mut rng = OsRng.unwrap_err();
    process_prekey_bundle(
        remote_address,
        &mut db.clone(),
        &mut db.clone(),
        &bundle,
        SystemTime::now(),
        &mut rng,
    )
    .await
    .map_err(|err| KursalError::Crypto(err.to_string()))?;

    Ok(())
}
