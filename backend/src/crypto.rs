//! Encryption-at-rest for organisation-owned secrets (native chat feature,
//! Phase 1). No reversible-encryption pattern existed anywhere in this
//! codebase before this feature — `argon2` (auth.rs) is a one-way password
//! KDF and cannot be reused here, since an organisation's OpenAI API key
//! must later be *decrypted* to actually call OpenAI, unlike a password
//! hash which is only ever compared, never recovered. This module is the
//! only place in the backend that touches a raw, plaintext organisation API
//! key outside of the one request that calls the provider with it.
//!
//! AES-256-GCM, via the `aes-gcm` crate: authenticated encryption (a
//! tampered ciphertext fails to decrypt rather than silently decrypting to
//! garbage), a 256-bit key, and a fresh random 96-bit nonce per encryption
//! call (never reused — reusing a nonce with the same key breaks GCM's
//! confidentiality guarantee entirely). The nonce is not a secret; it's
//! stored alongside the ciphertext (prefixed, hex-encoded together as one
//! string) so decryption never needs a second lookup.
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use rand::RngCore;

use crate::error::AppError;

const NONCE_LEN: usize = 12; // 96-bit GCM nonce

/// Parses `hex_key` (must be exactly 64 hex characters = 32 bytes) into an
/// AES-256 key. Called once per request that needs to encrypt/decrypt,
/// which is cheap enough (a handful of times a minute at this product's
/// scale) not to warrant caching a parsed key in `AppState`.
fn load_key(hex_key: &str) -> Result<Aes256Gcm, AppError> {
    let bytes = hex::decode(hex_key).map_err(|_| {
        AppError::Internal(
            "API_KEY_ENCRYPTION_KEY is not valid hex — see backend/.env.example.".to_string(),
        )
    })?;
    if bytes.len() != 32 {
        return Err(AppError::Internal(
            "API_KEY_ENCRYPTION_KEY must decode to exactly 32 bytes (64 hex characters) for \
             AES-256 — see backend/.env.example."
                .to_string(),
        ));
    }
    let key = Key::<Aes256Gcm>::from_slice(&bytes);
    Ok(Aes256Gcm::new(key))
}

/// Encrypts `plaintext` (a real, raw API key) under `hex_key`, returning a
/// single hex string: a fresh random nonce followed by the ciphertext
/// (which itself carries GCM's authentication tag). This is the only
/// representation ever written to `organization_api_keys.encrypted_key`.
pub fn encrypt_secret(plaintext: &str, hex_key: &str) -> Result<String, AppError> {
    let cipher = load_key(hex_key)?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| AppError::Internal(format!("failed to encrypt secret: {e}")))?;

    let mut combined = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);
    Ok(hex::encode(combined))
}

/// The inverse of `encrypt_secret`. Fails (rather than returning garbage) if
/// `hex_key` is wrong or `stored` has been tampered with — GCM's
/// authentication tag makes both cases indistinguishable from each other,
/// which is fine here: either way the caller cannot trust the result, so
/// there is no case where partially trusting a failed decrypt is useful.
pub fn decrypt_secret(stored: &str, hex_key: &str) -> Result<String, AppError> {
    let cipher = load_key(hex_key)?;
    let combined = hex::decode(stored)
        .map_err(|_| AppError::Internal("stored API key ciphertext is not valid hex.".to_string()))?;
    if combined.len() <= NONCE_LEN {
        return Err(AppError::Internal("stored API key ciphertext is too short.".to_string()));
    }
    let (nonce_bytes, ciphertext) = combined.split_at(NONCE_LEN);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| AppError::Internal("failed to decrypt stored API key.".to_string()))?;
    String::from_utf8(plaintext)
        .map_err(|_| AppError::Internal("decrypted API key was not valid UTF-8.".to_string()))
}

/// The last 4 characters of a real key, for the masked confirmation string
/// the UI shows once a key is saved (Phase 3) — e.g. `sk-...ab12`. Never
/// requires decrypting the stored ciphertext to render, since this is
/// computed once at provisioning time and stored in its own plaintext
/// `last_four` column (see migration 0017) — a 4-character fragment on its
/// own is not a meaningful secret.
pub fn last_four(plaintext: &str) -> String {
    let trimmed = plaintext.trim();
    if trimmed.len() <= 4 {
        trimmed.to_string()
    } else {
        trimmed[trimmed.len() - 4..].to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> String {
        "a".repeat(64) // 64 hex chars = 32 bytes of 0xaa
    }

    #[test]
    fn round_trips_a_real_looking_openai_key() {
        let plaintext = "sk-proj-abcdefghijklmnopqrstuvwxyz0123456789ABCD";
        let encrypted = encrypt_secret(plaintext, &test_key()).unwrap();
        assert_ne!(encrypted, plaintext, "ciphertext must not equal the plaintext");
        assert!(!encrypted.contains("sk-proj"), "ciphertext must not leak the plaintext prefix");
        let decrypted = decrypt_secret(&encrypted, &test_key()).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn two_encryptions_of_the_same_plaintext_produce_different_ciphertext() {
        // Proves the nonce is genuinely random per call, not reused — a
        // reused nonce under the same key is a real, well-known GCM
        // confidentiality break, not a theoretical one.
        let plaintext = "sk-test-key-1234567890";
        let a = encrypt_secret(plaintext, &test_key()).unwrap();
        let b = encrypt_secret(plaintext, &test_key()).unwrap();
        assert_ne!(a, b);
        assert_eq!(decrypt_secret(&a, &test_key()).unwrap(), plaintext);
        assert_eq!(decrypt_secret(&b, &test_key()).unwrap(), plaintext);
    }

    #[test]
    fn decrypting_with_the_wrong_key_fails_closed() {
        let plaintext = "sk-secret";
        let encrypted = encrypt_secret(plaintext, &test_key()).unwrap();
        let wrong_key = "b".repeat(64);
        assert!(decrypt_secret(&encrypted, &wrong_key).is_err());
    }

    #[test]
    fn tampered_ciphertext_fails_to_decrypt_rather_than_returning_garbage() {
        let plaintext = "sk-secret-value";
        let mut encrypted = encrypt_secret(plaintext, &test_key()).unwrap();
        // Flip a hex character well inside the ciphertext portion (past the
        // 24-hex-char nonce prefix).
        let mid = encrypted.len() / 2;
        let flipped = if &encrypted[mid..mid + 1] == "0" { "1" } else { "0" };
        encrypted.replace_range(mid..mid + 1, flipped);
        assert!(decrypt_secret(&encrypted, &test_key()).is_err());
    }

    #[test]
    fn last_four_masks_correctly() {
        assert_eq!(last_four("sk-proj-abcdefgh1234"), "1234");
        assert_eq!(last_four("ab"), "ab");
    }
}
