// Encrypted credential management.
//
// During `pesaraja init`, Daraja credentials (consumer key, secret, passkey, etc.)
// are encrypted with AES-256-GCM and stored in the platform config directory.
//
// The SDK key (`dsk_test_...` / `dsk_live_...`) IS the AES decryption key,
// base64-encoded with an environment prefix.

use crate::error::DarajaError;
use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, KeyInit},
};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rand::RngCore;
use rand::rngs::OsRng;
use std::path::PathBuf;
use zeroize::Zeroizing;

/// Return the application configuration directory.
///
/// `DARAJA_CONFIG_DIR` takes precedence. Otherwise this follows the platform's
/// conventional config root (`XDG_CONFIG_HOME` on Unix and `APPDATA` on Windows).
pub fn daraja_config_dir() -> Result<PathBuf, DarajaError> {
    if let Some(path) = std::env::var_os("DARAJA_CONFIG_DIR").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }

    #[cfg(windows)]
    if let Some(path) = std::env::var_os("APPDATA").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path).join("daraja"));
    }

    #[cfg(not(windows))]
    if let Some(path) = std::env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path).join("daraja"));
    }

    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            DarajaError::Config("could not determine the user config directory".into())
        })?;
    #[cfg(windows)]
    return Ok(PathBuf::from(home)
        .join("AppData")
        .join("Roaming")
        .join("daraja"));
    #[cfg(not(windows))]
    Ok(PathBuf::from(home).join(".config").join("daraja"))
}

/// Internal struct for JSON (de)serialization of credentials (avoids requiring
/// Serialize/Deserialize on the public Zeroizing-wrapped Credentials).
#[derive(serde::Serialize, serde::Deserialize)]
struct PlainCreds {
    consumer_key: String,
    consumer_secret: String,
    passkey: String,
    shortcode: String,
    initiator_name: Option<String>,
    initiator_password: Option<String>,
}

/// Plaintext credentials held in memory (zeroized on drop).
#[derive(Clone)]
pub struct Credentials {
    pub consumer_key: Zeroizing<String>,
    pub consumer_secret: Zeroizing<String>,
    pub passkey: Zeroizing<String>,
    pub shortcode: String,
    pub initiator_name: Option<Zeroizing<String>>,
    pub initiator_password: Option<Zeroizing<String>>,
}

/// Generate a new random 256-bit AES key, then encode it with the given prefix.
///
/// Returns: `dsk_test_<base64>` or `dsk_live_<base64>`.
pub fn generate_sdk_key(prefix: &str) -> String {
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    let b64 = STANDARD.encode(key);
    format!("dsk_{prefix}_") + &b64
}

/// Encrypt credentials with AES-256-GCM using the given 256-bit key.
///
/// Returns the encrypted blob bytes.
pub fn encrypt_credentials(creds: &Credentials, key: &[u8; 32]) -> Result<Vec<u8>, DarajaError> {
    let plain = PlainCreds {
        consumer_key: creds.consumer_key.to_string(),
        consumer_secret: creds.consumer_secret.to_string(),
        passkey: creds.passkey.to_string(),
        shortcode: creds.shortcode.clone(),
        initiator_name: creds.initiator_name.as_deref().map(|s| s.to_string()),
        initiator_password: creds.initiator_password.as_deref().map(|s| s.to_string()),
    };

    let plaintext = serde_json::to_vec(&plain)
        .map_err(|e| DarajaError::Encryption(format!("serialize failed: {e}")))?;

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_ref())
        .map_err(|e| DarajaError::Encryption(format!("encrypt failed: {e}")))?;

    let mut result = nonce_bytes.to_vec();
    result.extend(ciphertext);
    Ok(result)
}

/// Decrypt an encrypted blob with AES-256-GCM using the given 256-bit key.
///
/// Returns the plaintext credentials on success, or an error if the key is wrong
/// or the data has been tampered with.
pub fn decrypt_credentials(encrypted: &[u8], key: &[u8; 32]) -> Result<Credentials, DarajaError> {
    if encrypted.len() < 12 {
        return Err(DarajaError::Encryption("encrypted data too short".into()));
    }

    let (nonce_bytes, ciphertext) = encrypted.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));

    let plaintext = cipher.decrypt(nonce, ciphertext).map_err(|_| {
        DarajaError::Encryption("decryption failed (wrong key or tampered data)".into())
    })?;

    let plain: PlainCreds = serde_json::from_slice(&plaintext)
        .map_err(|e| DarajaError::Encryption(format!("deserialize failed: {e}")))?;

    Ok(Credentials {
        consumer_key: Zeroizing::new(plain.consumer_key),
        consumer_secret: Zeroizing::new(plain.consumer_secret),
        passkey: Zeroizing::new(plain.passkey),
        shortcode: plain.shortcode,
        initiator_name: plain.initiator_name.map(Zeroizing::new),
        initiator_password: plain.initiator_password.map(Zeroizing::new),
    })
}

/// Extract the raw 256-bit key bytes from an SDK key string.
///
/// Strips the `dsk_test_` or `dsk_live_` prefix, then base64-decodes the remainder.
pub fn extract_key_from_sdk_key(sdk_key: &str) -> Result<[u8; 32], DarajaError> {
    let b64_part = if let Some(rest) = sdk_key.strip_prefix("dsk_test_") {
        rest
    } else if let Some(rest) = sdk_key.strip_prefix("dsk_live_") {
        rest
    } else {
        return Err(DarajaError::Config(
            "SDK key must start with dsk_test_ or dsk_live_".into(),
        ));
    };

    let bytes = STANDARD
        .decode(b64_part)
        .map_err(|e| DarajaError::Config(format!("invalid base64 in SDK key: {e}")))?;

    if bytes.len() != 32 {
        return Err(DarajaError::Config(
            "SDK key must decode to exactly 32 bytes".into(),
        ));
    }

    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── generate_sdk_key ─────────────────────────────────────

    #[test]
    fn sdk_key_has_prefix() {
        let key = generate_sdk_key("test");
        assert!(
            key.starts_with("dsk_test_"),
            "key should start with dsk_test_"
        );
    }

    #[test]
    fn sdk_key_has_live_prefix() {
        let key = generate_sdk_key("live");
        assert!(
            key.starts_with("dsk_live_"),
            "key should start with dsk_live_"
        );
    }

    #[test]
    fn sdk_key_is_base64_after_prefix() {
        let key = generate_sdk_key("test");
        let b64_part = key.strip_prefix("dsk_test_").unwrap();
        // Base64-encoded 32 bytes = 44 characters with padding
        assert_eq!(b64_part.len(), 44, "base64 of 32 bytes = 44 chars");
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64_part)
            .expect("suffix should be valid base64");
    }

    #[test]
    fn each_generated_key_is_unique() {
        let k1 = generate_sdk_key("test");
        let k2 = generate_sdk_key("test");
        let k3 = generate_sdk_key("test");
        assert_ne!(k1, k2);
        assert_ne!(k1, k3);
        assert_ne!(k2, k3);
    }

    #[test]
    fn extracted_key_is_32_bytes() {
        let sdk_key = generate_sdk_key("test");
        let key_bytes = extract_key_from_sdk_key(&sdk_key).expect("valid SDK key");
        assert_eq!(
            key_bytes.len(),
            32,
            "key must be exactly 32 bytes (256 bits)"
        );
    }

    // ── encrypt / decrypt roundtrip ──────────────────────────

    fn make_test_creds() -> Credentials {
        Credentials {
            consumer_key: Zeroizing::new("ck_test_abc123".into()),
            consumer_secret: Zeroizing::new("cs_secret_xyz".into()),
            passkey: Zeroizing::new(
                "bfb279f9aa9bdbcf158e97dd71a467cd2e0c893059b10f78e6b72ada1ed2c919".into(),
            ),
            shortcode: "174379".into(),
            initiator_name: Some(Zeroizing::new("apitest".into())),
            initiator_password: Some(Zeroizing::new("Safaricom999!*!".into())),
        }
    }

    #[test]
    fn roundtrip_encrypt_decrypt_preserves_credentials() {
        let creds = make_test_creds();
        let sdk_key = generate_sdk_key("test");
        let key_bytes = extract_key_from_sdk_key(&sdk_key).unwrap();

        let encrypted = encrypt_credentials(&creds, &key_bytes).expect("encryption should succeed");
        let decrypted =
            decrypt_credentials(&encrypted, &key_bytes).expect("decryption should succeed");

        assert_eq!(*decrypted.consumer_key, *creds.consumer_key);
        assert_eq!(*decrypted.consumer_secret, *creds.consumer_secret);
        assert_eq!(*decrypted.passkey, *creds.passkey);
        assert_eq!(decrypted.shortcode, creds.shortcode);
        assert_eq!(
            decrypted.initiator_name.as_deref().map(|s| s.as_str()),
            creds.initiator_name.as_deref().map(|s| s.as_str())
        );
    }

    #[test]
    fn wrong_key_fails_decryption() {
        let creds = make_test_creds();
        let key1 = extract_key_from_sdk_key(&generate_sdk_key("test")).unwrap();
        let key2 = extract_key_from_sdk_key(&generate_sdk_key("test")).unwrap();

        let encrypted = encrypt_credentials(&creds, &key1).unwrap();
        let result = decrypt_credentials(&encrypted, &key2);

        assert!(result.is_err(), "decryption with wrong key must fail");
    }

    #[test]
    fn tampered_blob_fails_decryption() {
        let creds = make_test_creds();
        let key = extract_key_from_sdk_key(&generate_sdk_key("test")).unwrap();

        let mut encrypted = encrypt_credentials(&creds, &key).unwrap();
        // Tamper with a byte
        let len = encrypted.len();
        if len > 0 {
            encrypted[len / 2] ^= 0xFF;
        }

        let result = decrypt_credentials(&encrypted, &key);
        assert!(
            result.is_err(),
            "tampered ciphertext must fail AES-GCM authentication"
        );
    }

    #[test]
    fn empty_blob_fails_decryption() {
        let key = extract_key_from_sdk_key(&generate_sdk_key("test")).unwrap();
        let result = decrypt_credentials(&[], &key);
        assert!(result.is_err(), "empty blob should error");
    }

    // ── extract_key_from_sdk_key ────────────────────────────

    #[test]
    fn extract_key_fails_on_invalid_prefix() {
        // Random string without dsk_ prefix
        let result = extract_key_from_sdk_key("not-a-valid-key");
        assert!(result.is_err());
    }

    #[test]
    fn extract_key_fails_on_garbled_base64() {
        let result = extract_key_from_sdk_key("dsk_test_!!!not-base64!!!");
        assert!(result.is_err());
    }

    #[test]
    fn extract_key_fails_on_wrong_length_after_decode() {
        // Base64 of 16 bytes = 24 chars, but we need 44 (for 32 bytes)
        let short_b64 =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, [0u8; 16]);
        let result = extract_key_from_sdk_key(&format!("dsk_test_{short_b64}"));
        assert!(result.is_err(), "16-byte key should be rejected (need 32)");
    }

    // ── Credentials zeroize behavior ────────────────────────

    #[test]
    fn credentials_struct_exists_and_compiles() {
        let creds = make_test_creds();
        assert_eq!(*creds.consumer_key, "ck_test_abc123");
        assert_eq!(creds.shortcode, "174379");
    }

    // ── Edge cases ───────────────────────────────────────────

    #[test]
    fn encrypt_same_data_with_same_key_produces_different_blobs() {
        let creds = make_test_creds();
        let key = extract_key_from_sdk_key(&generate_sdk_key("test")).unwrap();
        let blob1 = encrypt_credentials(&creds, &key).unwrap();
        let blob2 = encrypt_credentials(&creds, &key).unwrap();
        assert_ne!(
            blob1, blob2,
            "different nonces must produce different ciphertexts"
        );
    }

    #[test]
    fn decrypt_truncated_data_fails() {
        let creds = make_test_creds();
        let key = extract_key_from_sdk_key(&generate_sdk_key("test")).unwrap();
        let encrypted = encrypt_credentials(&creds, &key).unwrap();
        // Truncate to just the nonce (12 bytes)
        let result = decrypt_credentials(&encrypted[..12], &key);
        assert!(result.is_err());
    }

    #[test]
    fn decrypt_exactly_12_bytes_fails() {
        let key = extract_key_from_sdk_key(&generate_sdk_key("test")).unwrap();
        let result = decrypt_credentials(&[0u8; 12], &key);
        assert!(
            result.is_err(),
            "nonce-only data should fail (no ciphertext)"
        );
    }

    #[test]
    fn decrypt_13_bytes_fails() {
        let key = extract_key_from_sdk_key(&generate_sdk_key("test")).unwrap();
        let result = decrypt_credentials(&[0u8; 13], &key);
        assert!(
            result.is_err(),
            "1 byte of fake ciphertext should fail auth"
        );
    }

    #[test]
    fn extract_key_fails_on_empty_suffix() {
        let result = extract_key_from_sdk_key("dsk_test_");
        assert!(result.is_err(), "empty base64 after prefix should fail");
    }

    #[test]
    fn extract_key_fails_on_prefix_only() {
        let result = extract_key_from_sdk_key("dsk_test_");
        assert!(result.is_err());
        let result = extract_key_from_sdk_key("dsk_live_");
        assert!(result.is_err());
    }

    #[test]
    fn credentials_with_unicode_roundtrips() {
        let creds = Credentials {
            consumer_key: Zeroizing::new("🔑-key-🚀".into()),
            consumer_secret: Zeroizing::new("🔒-secret-💥".into()),
            passkey: Zeroizing::new("abc123".into()),
            shortcode: "174379".into(),
            initiator_name: None,
            initiator_password: None,
        };
        let key = extract_key_from_sdk_key(&generate_sdk_key("test")).unwrap();
        let encrypted = encrypt_credentials(&creds, &key).unwrap();
        let decrypted = decrypt_credentials(&encrypted, &key).unwrap();
        assert_eq!(*decrypted.consumer_key, *creds.consumer_key);
        assert_eq!(*decrypted.consumer_secret, *creds.consumer_secret);
    }

    #[test]
    fn credentials_without_optional_fields_roundtrips() {
        let creds = Credentials {
            consumer_key: Zeroizing::new("ck".into()),
            consumer_secret: Zeroizing::new("cs".into()),
            passkey: Zeroizing::new("pk".into()),
            shortcode: "123".into(),
            initiator_name: None,
            initiator_password: None,
        };
        let key = extract_key_from_sdk_key(&generate_sdk_key("test")).unwrap();
        let encrypted = encrypt_credentials(&creds, &key).unwrap();
        let decrypted = decrypt_credentials(&encrypted, &key).unwrap();
        assert_eq!(decrypted.initiator_name, None);
        assert_eq!(decrypted.initiator_password, None);
    }

    #[test]
    fn encrypt_empty_credentials_works() {
        let creds = Credentials {
            consumer_key: Zeroizing::new(String::new()),
            consumer_secret: Zeroizing::new(String::new()),
            passkey: Zeroizing::new(String::new()),
            shortcode: String::new(),
            initiator_name: None,
            initiator_password: None,
        };
        let key = extract_key_from_sdk_key(&generate_sdk_key("test")).unwrap();
        let encrypted = encrypt_credentials(&creds, &key).unwrap();
        let decrypted = decrypt_credentials(&encrypted, &key).unwrap();
        assert_eq!(*decrypted.consumer_key, "");
        assert_eq!(decrypted.shortcode, "");
    }

    #[test]
    fn sdk_key_prefix_handles_custom_prefix() {
        let key = generate_sdk_key("custom");
        assert!(key.starts_with("dsk_custom_"));
    }
}
