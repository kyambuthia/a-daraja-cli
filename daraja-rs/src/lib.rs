// ─── Public API ───────────────────────────────────────────────
// The single entry point: `Daraja::from_env()` or `Daraja::from_key(key)`

pub mod auth;
pub mod client;
pub mod config;
pub mod error;
pub mod security;
pub mod types;
pub mod webhook;

pub mod api;

use error::DarajaError;

/// Main entry point. Holds in-memory credentials and an HTTP client.
pub struct Daraja;

impl Daraja {
    /// Create from `DARAJA_KEY` env var.
    pub fn from_env() -> Result<Self, DarajaError> {
        todo!()
    }

    /// Create from an explicit SDK key string.
    pub fn from_key(_key: &str) -> Result<Self, DarajaError> {
        todo!()
    }
}

// Tests are co-located in each module with #[cfg(test)].
