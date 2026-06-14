// ─── Public API ───────────────────────────────────────────────
// The single entry point: `Daraja::from_env()` or `Daraja::from_key(key)`

pub mod auth;
pub mod client;
pub mod config;
pub mod error;
pub mod idempotency;
pub mod security;
pub mod types;
pub mod webhook;

pub mod api;

use client::DarajaClient;
use error::DarajaError;
use types::Environment;

/// Main entry point. Holds an HTTP client configured with credentials.
pub struct Daraja {
    client: DarajaClient,
}

impl Daraja {
    /// Create from `DARAJA_KEY` env var.
    pub fn from_env() -> Result<Self, DarajaError> {
        let key = std::env::var("DARAJA_KEY")
            .map_err(|_| DarajaError::MissingApiKey)?;
        Self::from_key(&key)
    }

    /// Create from an explicit SDK key string.
    pub fn from_key(key: &str) -> Result<Self, DarajaError> {
        let key_bytes = config::extract_key_from_sdk_key(key)?;

        let home = std::env::var("HOME")
            .map_err(|_| DarajaError::Config("HOME environment variable is not set".into()))?;
        let config_path = format!("{}/.config/daraja/credentials.enc", home);

        let encrypted_bytes = std::fs::read(&config_path).map_err(|e| {
            DarajaError::Config(format!(
                "failed to read credentials file at {}: {}",
                config_path, e
            ))
        })?;

        let credentials = config::decrypt_credentials(&encrypted_bytes, &key_bytes)?;

        let environment = if key.starts_with("dsk_test_") {
            Environment::Sandbox
        } else {
            Environment::Production
        };

        let client = DarajaClient::new(credentials, environment)?;
        Ok(Daraja { client })
    }

    /// Access the underlying client for direct API calls.
    pub fn client(&self) -> &DarajaClient {
        &self.client
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_env_missing_key_is_error() {
        // Ensure DARAJA_KEY is not set in test env
        unsafe { std::env::remove_var("DARAJA_KEY"); }
        let result = Daraja::from_env();
        assert!(result.is_err());
    }

    #[test]
    fn from_env_reads_key() {
        let key = format!("dsk_test_{}", "a".repeat(44));
        unsafe { std::env::set_var("DARAJA_KEY", key); }
        let result = Daraja::from_env();
        // Should fail on key parsing (invalid base64), not on missing env var
        assert!(result.is_err());
        // The error should NOT be MissingApiKey
        match result {
            Err(DarajaError::MissingApiKey) => panic!("should not be missing key"),
            _ => {} // expected — key parse failure or no creds file
        }
        unsafe { std::env::remove_var("DARAJA_KEY"); }
    }

    #[test]
    fn from_key_rejects_invalid_key() {
        let result = Daraja::from_key("not-a-valid-key");
        assert!(result.is_err());
    }

    #[test]
    fn from_key_accepts_valid_test_key_format() {
        let key = config::generate_sdk_key("test");
        let result = Daraja::from_key(&key);
        // Will fail on missing credentials file, not on key format
        match result {
            Err(DarajaError::Config(_)) => {} // expected: no creds file
            Err(DarajaError::Encryption(_)) => {} // also possible
            Err(e) => panic!("unexpected error: {:?}", e),
            Ok(_) => panic!("should fail without credentials file"),
        }
    }
}
