// HTTP client wrapper with OAuth, retry, and idempotency.
//
// Every outgoing request:
// 1. Ensures a valid OAuth token is cached
// 2. Attaches an Idempotency-Key header (UUID v4)
// 3. Tracks the key in a local cache to prevent double-sends
// 4. Retries on transient failures with exponential backoff

use crate::auth::TokenManager;
use crate::config::Credentials;
use crate::error::DarajaError;
use crate::types::Environment;

/// The HTTP client that all API calls go through.
#[allow(dead_code)]
pub struct DarajaClient {
    inner: reqwest::Client,
    token_manager: TokenManager,
    environment: Environment,
}

impl DarajaClient {
    /// Create a new client with credentials and environment.
    pub fn new(_credentials: Credentials, _environment: Environment) -> Result<Self, DarajaError> {
        todo!()
    }

    /// Send a POST request to a Daraja endpoint.
    /// Handles OAuth token injection, idempotency, and retries.
    pub async fn post<T: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        _path: &str,
        _body: &T,
    ) -> Result<R, DarajaError> {
        todo!()
    }

    /// Send a GET request to a Daraja endpoint.
    pub async fn get<R: serde::de::DeserializeOwned>(
        &self,
        _path: &str,
    ) -> Result<R, DarajaError> {
        todo!()
    }
}

// ── Idempotency ──────────────────────────────────────────────

use uuid::Uuid;

/// Generate a new idempotency key (UUID v4).
pub fn generate_idempotency_key() -> String {
    Uuid::new_v4().to_string()
}

/// Check whether an idempotency key is valid (UUID v4 format).
pub fn is_valid_idempotency_key(key: &str) -> bool {
    if key.is_empty() {
        return false;
    }
    match Uuid::parse_str(key) {
        Ok(uuid) => !uuid.is_nil(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── generate_idempotency_key ────────────────────────────

    #[test]
    fn key_is_a_valid_uuid_v4() {
        let key = generate_idempotency_key();
        let parsed = Uuid::parse_str(&key).expect("should be a valid UUID");
        assert_eq!(parsed.get_version_num(), 4, "should be UUID v4");
    }

    #[test]
    fn each_key_is_unique() {
        let k1 = generate_idempotency_key();
        let k2 = generate_idempotency_key();
        let k3 = generate_idempotency_key();
        assert_ne!(k1, k2);
        assert_ne!(k1, k3);
        assert_ne!(k2, k3);
    }

    #[test]
    fn keys_are_36_characters() {
        for _ in 0..10 {
            let key = generate_idempotency_key();
            assert_eq!(key.len(), 36, "UUID v4 string is 36 chars with hyphens");
        }
    }

    // ── is_valid_idempotency_key ────────────────────────────

    #[test]
    fn valid_uuid_v4_passes_validation() {
        let key = generate_idempotency_key();
        assert!(is_valid_idempotency_key(&key));
    }

    #[test]
    fn random_string_fails_validation() {
        assert!(!is_valid_idempotency_key("not-a-uuid"));
    }

    #[test]
    fn empty_string_fails_validation() {
        assert!(!is_valid_idempotency_key(""));
    }

    #[test]
    fn nil_uuid_fails_validation() {
        // All-zeros UUID should probably not be accepted as a valid idempotency key
        // (it's a sentinel value, not a random key)
        assert!(!is_valid_idempotency_key("00000000-0000-0000-0000-000000000000"));
    }

    // ── Edge cases ───────────────────────────────────────────

    #[test]
    fn generate_1000_keys_all_unique() {
        let mut keys = Vec::with_capacity(1000);
        for _ in 0..1000 {
            keys.push(generate_idempotency_key());
        }
        let mut sorted = keys.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 1000, "1000 keys must all be unique");
    }

    #[test]
    fn valid_uuid_v1_passes_validation() {
        // We validate format, not version. A v1 UUID is still a valid UUID.
        let v1 = "c9c5b6e0-4b3a-11ee-be56-0242ac120002";
        assert!(is_valid_idempotency_key(v1));
    }

    #[test]
    fn mixed_case_uuid_passes_validation() {
        // Uuid::parse_str is case-insensitive per RFC 9562
        let key = generate_idempotency_key().to_uppercase();
        assert!(is_valid_idempotency_key(&key), "UUID parse is case-insensitive");
    }

    #[test]
    fn uuid_with_extra_chars_fails_validation() {
        let mut key = generate_idempotency_key();
        key.push('x');
        assert!(!is_valid_idempotency_key(&key));
    }

    #[test]
    fn uuid_with_missing_char_fails_validation() {
        let key = "550e8400-e29b-41d4-a716-44665544000"; // 35 chars, missing one
        assert!(!is_valid_idempotency_key(key));
    }

    #[test]
    fn generate_key_always_v4() {
        for _ in 0..50 {
            let key = generate_idempotency_key();
            let uuid = Uuid::parse_str(&key).unwrap();
            assert_eq!(uuid.get_version_num(), 4);
        }
    }
}
