use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};
use hkdf::Hkdf;
use rand::{TryRngCore, rngs::OsRng};
use sha2::Sha256;

use crate::{KursalError, Result};

pub fn derive_stream_key(my_random: [u8; 32], their_random: [u8; 32]) -> [u8; 32] {
    let xor: Vec<_> = my_random
        .iter()
        .zip(their_random)
        .map(|(x, y)| x ^ y)
        .collect();

    let hk = Hkdf::<Sha256>::new(None, &xor);
    let mut key = [0u8; 32];

    hk.expand(b"kursal-stream-key", &mut key).unwrap();

    key
}

pub fn stream_encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|err| KursalError::Crypto(err.to_string()))?;

    let mut nonce = [0u8; 12];
    OsRng
        .try_fill_bytes(&mut nonce)
        .map_err(|err| KursalError::Crypto(err.to_string()))?;

    let encrypted = cipher
        .encrypt(&nonce.into(), plaintext)
        .map_err(|err| KursalError::Crypto(err.to_string()))?;

    // Prepend nonce
    let mut output = Vec::with_capacity(12 + encrypted.len());
    output.extend_from_slice(&nonce);
    output.extend_from_slice(&encrypted);

    Ok(output)
}

pub fn stream_decrypt(key: &[u8; 32], ciphertext: &[u8]) -> Result<Vec<u8>> {
    // must be at least 28 bytes, 12 nonce + 16 auth
    if ciphertext.len() < 28 {
        return Err(KursalError::Crypto(
            "Ciphertext must be at least 28 bytes".to_string(),
        ));
    }

    let (nonce, cipherinput) = ciphertext.split_at(12);

    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|err| KursalError::Crypto(err.to_string()))?;

    let decrypted = cipher
        .decrypt(nonce.into(), cipherinput)
        .map_err(|err| KursalError::Crypto(err.to_string()))?;

    Ok(decrypted)
}
