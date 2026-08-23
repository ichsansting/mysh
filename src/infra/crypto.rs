use crate::error::{Error, Result};
use argon2::Argon2;
use chacha20poly1305::aead::Aead;
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use std::path::Path;

// Envelope layout: `salt(16) || nonce(24) || ciphertext+tag`. This is the v1 on-disk
// format of every existing `.age` file — it MUST NOT change, or real Secrets already
// committed to Source become undecryptable.
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 24;
const KEY_LEN: usize = 32;

fn derive_key(passphrase: &str, salt: &[u8], path: &Path) -> Result<[u8; KEY_LEN]> {
    let mut key = [0u8; KEY_LEN];
    Argon2::default()
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|e| Error::Crypto {
            path: path.to_path_buf(),
            detail: format!("key derivation failed: {e}"),
        })?;
    Ok(key)
}

/// Encrypts `plaintext` for storage in Source. Fresh random salt and nonce per call,
/// so identical plaintext never produces identical ciphertext. `path` is only for
/// error reporting.
pub fn encrypt(plaintext: &[u8], passphrase: &str, path: &Path) -> Result<Vec<u8>> {
    let fail = |detail: String| Error::Crypto {
        path: path.to_path_buf(),
        detail,
    };
    let mut salt = [0u8; SALT_LEN];
    getrandom::fill(&mut salt).map_err(|e| fail(e.to_string()))?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce_bytes).map_err(|e| fail(e.to_string()))?;

    let key = derive_key(passphrase, &salt, path)?;
    let cipher = XChaCha20Poly1305::new_from_slice(&key).map_err(|e| fail(e.to_string()))?;
    let ciphertext = cipher
        .encrypt(&XNonce::from(nonce_bytes), plaintext)
        .map_err(|_| fail("failed to encrypt".into()))?;

    let mut envelope = Vec::with_capacity(SALT_LEN + NONCE_LEN + ciphertext.len());
    envelope.extend_from_slice(&salt);
    envelope.extend_from_slice(&nonce_bytes);
    envelope.extend_from_slice(&ciphertext);
    Ok(envelope)
}

/// Decrypts an `encrypt`-produced envelope. A wrong passphrase and a corrupted file
/// are indistinguishable behind the AEAD tag, so the error deliberately says neither.
pub fn decrypt(envelope: &[u8], passphrase: &str, path: &Path) -> Result<Vec<u8>> {
    let fail = |detail: &str| Error::Crypto {
        path: path.to_path_buf(),
        detail: detail.into(),
    };
    if envelope.len() < SALT_LEN + NONCE_LEN {
        return Err(fail("too short to be a valid secret envelope"));
    }
    let (salt, rest) = envelope.split_at(SALT_LEN);
    let (nonce_bytes, ciphertext) = rest.split_at(NONCE_LEN);

    let key = derive_key(passphrase, salt, path)?;
    let cipher = XChaCha20Poly1305::new_from_slice(&key).map_err(|e| Error::Crypto {
        path: path.to_path_buf(),
        detail: e.to_string(),
    })?;
    let nonce = XNonce::try_from(nonce_bytes).expect("nonce_bytes is NONCE_LEN long");
    cipher
        .decrypt(&nonce, ciphertext)
        .map_err(|_| fail("wrong passphrase or corrupted file"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: &str = "correct horse battery staple";

    fn at() -> &'static Path {
        Path::new("test.age")
    }

    #[test]
    fn encrypt_decrypt_round_trip() {
        let envelope = encrypt(b"secret\n", P, at()).unwrap();
        assert_ne!(envelope, b"secret\n");
        assert_eq!(decrypt(&envelope, P, at()).unwrap(), b"secret\n");
    }

    #[test]
    fn decrypt_with_wrong_passphrase_fails() {
        let envelope = encrypt(b"top secret", P, at()).unwrap();
        assert!(decrypt(&envelope, "wrong", at()).is_err());
    }

    #[test]
    fn encrypting_the_same_plaintext_twice_yields_different_ciphertext() {
        assert_ne!(
            encrypt(b"same", P, at()).unwrap(),
            encrypt(b"same", P, at()).unwrap()
        );
    }
}
