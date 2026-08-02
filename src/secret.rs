use argon2::Argon2;
use chacha20poly1305::aead::Aead;
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The Source-side suffix that marks a file as a `Secret`.
pub const SUFFIX: &str = "age";

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 24;
const KEY_LEN: usize = 32;

/// A closure that returns the shared Passphrase, prompting interactively (and caching
/// only for the lifetime of the current command) the first time it's actually needed.
/// Commands that touch no `Secret` never call it, so they're never prompted.
pub type PassphraseFn<'a> = dyn FnMut() -> Result<String, String> + 'a;

/// Builds a `PassphraseFn`: returns `configured` (from `--passphrase`/`MYSH_PASSPHRASE`)
/// if set, otherwise prompts on the terminal (without echoing input) the first time it's
/// called and reuses that answer for the rest of the process — never written to disk, a
/// keychain, or any agent.
pub fn passphrase_provider(configured: Option<String>) -> impl FnMut() -> Result<String, String> {
    let mut cached = configured;
    move || {
        if let Some(p) = &cached {
            return Ok(p.clone());
        }
        let entered = rpassword::prompt_password("mysh passphrase: ").map_err(|e| e.to_string())?;
        cached = Some(entered.clone());
        Ok(entered)
    }
}

/// Whether a Source-relative path names a `Secret` (a file ending in `.age`).
pub fn is_secret(relative: &Path) -> bool {
    relative.extension().is_some_and(|ext| ext == SUFFIX)
}

/// A Secret's rendered `Target` path: `relative` with its `.age` suffix stripped.
pub fn strip_suffix(relative: &Path) -> PathBuf {
    relative.with_extension("")
}

fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; KEY_LEN], String> {
    let mut key = [0u8; KEY_LEN];
    Argon2::default()
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|e| format!("failed to derive key: {e}"))?;
    Ok(key)
}

/// Encrypts `plaintext` for storage in `Source`. A fresh random salt and nonce are drawn
/// on every call, so encrypting identical plaintext twice never produces identical
/// ciphertext. Layout: `salt(16) || nonce(24) || ciphertext+tag`.
pub fn encrypt(plaintext: &[u8], passphrase: &str) -> Result<Vec<u8>, String> {
    let mut salt = [0u8; SALT_LEN];
    getrandom::fill(&mut salt).map_err(|e| e.to_string())?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce_bytes).map_err(|e| e.to_string())?;

    let key = derive_key(passphrase, &salt)?;
    let cipher = XChaCha20Poly1305::new_from_slice(&key).map_err(|e| e.to_string())?;
    let nonce = XNonce::from(nonce_bytes);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|_| "failed to encrypt secret".to_string())?;

    let mut envelope = Vec::with_capacity(SALT_LEN + NONCE_LEN + ciphertext.len());
    envelope.extend_from_slice(&salt);
    envelope.extend_from_slice(&nonce_bytes);
    envelope.extend_from_slice(&ciphertext);
    Ok(envelope)
}

/// Decrypts an `encrypt`-produced envelope. Fails if `passphrase` is wrong or `envelope`
/// is corrupted/truncated — the AEAD tag makes the two indistinguishable, so the error
/// deliberately doesn't say which.
pub fn decrypt(envelope: &[u8], passphrase: &str) -> Result<Vec<u8>, String> {
    if envelope.len() < SALT_LEN + NONCE_LEN {
        return Err("secret file is too short to be a valid envelope".to_string());
    }
    let (salt, rest) = envelope.split_at(SALT_LEN);
    let (nonce_bytes, ciphertext) = rest.split_at(NONCE_LEN);

    let key = derive_key(passphrase, salt)?;
    let cipher = XChaCha20Poly1305::new_from_slice(&key).map_err(|e| e.to_string())?;
    let nonce = XNonce::try_from(nonce_bytes).expect("nonce_bytes is NONCE_LEN long");
    cipher
        .decrypt(&nonce, ciphertext)
        .map_err(|_| "failed to decrypt secret: wrong passphrase or corrupted file".to_string())
}

/// Writes a Secret's decrypted content to `dest` with restrictive permissions (owner
/// read/write only — no group/world access), so a decrypted key or credential is never
/// left world-readable on disk. Skips the write (but still enforces permissions) when
/// content is already up to date, matching the idempotence of plain-file apply.
pub fn write_restricted(dest: &Path, content: &[u8]) -> io::Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    if fs::read(dest).map(|existing| existing != content).unwrap_or(true) {
        create_restricted(dest, content)?;
    }
    // Also covers a pre-existing file at `dest` (e.g. backed up on first touch): its
    // original permissions aren't ours to preempt, so this chmod fixes them up right
    // after, on top of the create-time mode below covering every fresh write.
    set_restricted_permissions(dest)
}

#[cfg(unix)]
fn create_restricted(dest: &Path, content: &[u8]) -> io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(dest)?
        .write_all(content)
}

#[cfg(not(unix))]
fn create_restricted(dest: &Path, content: &[u8]) -> io::Result<()> {
    fs::write(dest, content)
}

#[cfg(unix)]
fn set_restricted_permissions(dest: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(dest, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_restricted_permissions(_dest: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_round_trip() {
        let plaintext = b"-----BEGIN OPENSSH PRIVATE KEY-----\nsecret\n";
        let envelope = encrypt(plaintext, "correct horse battery staple").unwrap();
        assert_ne!(envelope, plaintext);
        assert_eq!(decrypt(&envelope, "correct horse battery staple").unwrap(), plaintext);
    }

    #[test]
    fn decrypt_with_wrong_passphrase_fails() {
        let envelope = encrypt(b"top secret", "right passphrase").unwrap();
        assert!(decrypt(&envelope, "wrong passphrase").is_err());
    }

    #[test]
    fn encrypting_the_same_plaintext_twice_yields_different_ciphertext() {
        let a = encrypt(b"same plaintext", "passphrase").unwrap();
        let b = encrypt(b"same plaintext", "passphrase").unwrap();
        assert_ne!(a, b, "fresh salt/nonce per call must avoid identical envelopes");
    }

    #[test]
    fn is_secret_matches_only_the_age_suffix() {
        assert!(is_secret(Path::new("ssh/id_rsa.age")));
        assert!(!is_secret(Path::new("ssh/id_rsa")));
        assert!(!is_secret(Path::new("bashrc")));
    }

    #[test]
    fn strip_suffix_removes_only_the_age_extension() {
        assert_eq!(strip_suffix(Path::new("ssh/id_rsa.age")), Path::new("ssh/id_rsa"));
    }
}
