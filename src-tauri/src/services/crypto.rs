//! Passphrase encryption for backup exports (feature recommendation 3).
//!
//! Format: `MBMENC1` magic (7 bytes) + 16-byte Argon2 salt + 12-byte
//! AES-256-GCM nonce + ciphertext. The plaintext inside is the same JSON the
//! plaintext export writes, so `import_backup` transparently handles both
//! once the bytes are decrypted.
//!
//! Key derivation: Argon2id with the crate's default (recommended) parameters
//! — memory-hard, so a stolen file cannot be brute-forced cheaply.

use crate::error::{AppError, AppResult};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::Argon2;

const MAGIC: &[u8; 7] = b"MBMENC1";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

/// Whether the bytes look like an encrypted MBM backup.
pub fn is_encrypted(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}

/// Minimal passphrase policy — enough to stop point-and-click mistakes,
/// not a substitute for a real password policy on the user's side.
pub fn validate_passphrase(passphrase: &str) -> AppResult<()> {
    if passphrase.chars().count() < 8 {
        return Err(AppError::validation(
            "Passphrase must be at least 8 characters",
        ));
    }
    Ok(())
}

fn derive_key(passphrase: &str, salt: &[u8]) -> AppResult<[u8; 32]> {
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|e| AppError::internal(format!("Key derivation failed: {e}")))?;
    Ok(key)
}

fn random_bytes<const N: usize>() -> AppResult<[u8; N]> {
    let mut buf = [0u8; N];
    getrandom::getrandom(&mut buf)?;
    Ok(buf)
}

/// Encrypts the plaintext backup with a passphrase.
pub fn encrypt(plaintext: &[u8], passphrase: &str) -> AppResult<Vec<u8>> {
    let salt = random_bytes::<SALT_LEN>()?;
    let nonce_bytes = random_bytes::<NONCE_LEN>()?;
    let key = derive_key(passphrase, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| AppError::internal(format!("Invalid key length: {e}")))?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plaintext)
        .map_err(|e| AppError::internal(format!("Encryption failed: {e}")))?;

    let mut out = Vec::with_capacity(MAGIC.len() + SALT_LEN + NONCE_LEN + ciphertext.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypts an encrypted backup. A wrong passphrase fails the GCM tag
/// verification and surfaces as a clean validation error.
pub fn decrypt(bytes: &[u8], passphrase: &str) -> AppResult<Vec<u8>> {
    if !is_encrypted(bytes) {
        return Err(AppError::validation("File is not an encrypted MBM backup"));
    }
    let header = MAGIC.len() + SALT_LEN + NONCE_LEN;
    if bytes.len() <= header {
        return Err(AppError::validation("Encrypted backup file is truncated"));
    }
    let salt = &bytes[MAGIC.len()..MAGIC.len() + SALT_LEN];
    let nonce_bytes = &bytes[MAGIC.len() + SALT_LEN..header];
    let ciphertext = &bytes[header..];

    let key = derive_key(passphrase, salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| AppError::internal(format!("Invalid key length: {e}")))?;
    cipher
        .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
        .map_err(|_| {
            AppError::validation("Wrong passphrase or corrupted backup file")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let plain = b"{\"format_version\":1,\"profiles\":[]}".to_vec();
        let enc = encrypt(&plain, "correct horse battery").unwrap();
        assert!(is_encrypted(&enc));
        assert_ne!(enc, plain);
        let dec = decrypt(&enc, "correct horse battery").unwrap();
        assert_eq!(dec, plain);
    }

    #[test]
    fn wrong_passphrase_is_rejected() {
        let enc = encrypt(b"secret", "right passphrase").unwrap();
        let err = decrypt(&enc, "wrong passphrase").unwrap_err();
        assert!(err.to_string().contains("Wrong passphrase"));
    }

    #[test]
    fn salt_and_nonce_are_fresh_per_call() {
        let a = encrypt(b"same plaintext", "passphrase").unwrap();
        let b = encrypt(b"same plaintext", "passphrase").unwrap();
        assert_ne!(a, b, "fresh salt/nonce must change the ciphertext");
    }

    #[test]
    fn truncated_files_are_rejected() {
        let enc = encrypt(b"secret", "passphrase").unwrap();
        assert!(decrypt(&enc[..20], "passphrase").is_err());
        assert!(decrypt(b"not encrypted at all", "passphrase").is_err());
    }

    #[test]
    fn passphrase_min_length_is_enforced() {
        assert!(validate_passphrase("short").is_err());
        assert!(validate_passphrase("long enough").is_ok());
    }
}
