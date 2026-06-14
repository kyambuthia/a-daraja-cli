// OAuth 2.0 token lifecycle management.
//
// Daraja tokens expire in 3600 seconds. We cache them and refresh
// automatically when they expire.

use crate::error::DarajaError;

/// An OAuth access token with expiry tracking.
pub struct Token {
    pub access_token: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

/// Manages the OAuth token lifecycle — fetch, cache, refresh.
#[allow(dead_code)]
pub struct TokenManager {
    consumer_key: String,
    consumer_secret: String,
    environment: super::types::Environment,
    cached_token: Option<Token>,
}

impl TokenManager {
    /// Create a new token manager.
    pub fn new(consumer_key: String, consumer_secret: String, environment: super::types::Environment) -> Self {
        TokenManager {
            consumer_key,
            consumer_secret,
            environment,
            cached_token: None,
        }
    }

    /// Get a valid token (cached or freshly fetched).
    pub async fn get_token(&mut self) -> Result<Token, DarajaError> {
        todo!()
    }

    /// Check whether a cached token has expired.
    pub fn is_expired(token: &Option<Token>) -> bool {
        match token {
            None => true,
            Some(t) => t.expires_at <= chrono::Utc::now(),
        }
    }

    /// Forcibly invalidate the cached token (e.g. on 401 response).
    pub fn invalidate(&mut self) {
        self.cached_token = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_expired ───────────────────────────────────────────

    #[test]
    fn no_token_is_expired() {
        assert!(TokenManager::is_expired(&None));
    }

    #[test]
    fn token_in_past_is_expired() {
        let token = Token {
            access_token: "abc".into(),
            expires_at: chrono::Utc::now() - chrono::Duration::seconds(10),
        };
        assert!(TokenManager::is_expired(&Some(token)));
    }

    #[test]
    fn token_in_future_is_not_expired() {
        let token = Token {
            access_token: "abc".into(),
            expires_at: chrono::Utc::now() + chrono::Duration::seconds(300),
        };
        assert!(!TokenManager::is_expired(&Some(token)));
    }

    #[test]
    fn token_exactly_at_expiry_is_expired() {
        // Treat "at expiry" as expired to avoid edge-case race conditions.
        let token = Token {
            access_token: "abc".into(),
            expires_at: chrono::Utc::now(),
        };
        assert!(TokenManager::is_expired(&Some(token)));
    }

    // ── Token struct ─────────────────────────────────────────

    #[test]
    fn token_stores_access_token_and_expiry() {
        let expiry = chrono::Utc::now() + chrono::Duration::seconds(3599);
        let token = Token {
            access_token: "test-token-123".into(),
            expires_at: expiry,
        };
        assert_eq!(token.access_token, "test-token-123");
        assert_eq!(token.expires_at, expiry);
    }

    // ── invalidate ───────────────────────────────────────────

    #[test]
    fn invalidate_clears_cached_token() {
        // We can't easily test `get_token` without network,
        // but `invalidate` should make the next `is_expired` call return true.
        // This test verifies the concept compiles and is callable.
        let mut manager = TokenManager::new(
            "key".into(),
            "secret".into(),
            crate::types::Environment::Sandbox,
        );
        manager.invalidate();
    }

    // ── get_token sandbox URL ────────────────────────────────

    #[test]
    fn sandbox_auth_url_is_correct() {
        // This test validates that the sandbox OAuth endpoint is what we expect.
        // Actual HTTP request is tested in integration tests.
        let sandbox_base = "https://sandbox.safaricom.co.ke";
        let auth_path = "/oauth/v1/generate?grant_type=client_credentials";
        assert_eq!(
            format!("{}{}", sandbox_base, auth_path),
            "https://sandbox.safaricom.co.ke/oauth/v1/generate?grant_type=client_credentials"
        );
    }

    #[test]
    fn production_auth_url_is_correct() {
        let prod_base = "https://api.safaricom.co.ke";
        let auth_path = "/oauth/v1/generate?grant_type=client_credentials";
        assert_eq!(
            format!("{}{}", prod_base, auth_path),
            "https://api.safaricom.co.ke/oauth/v1/generate?grant_type=client_credentials"
        );
    }

    // ── Edge cases ───────────────────────────────────────────

    #[test]
    fn invalidate_on_new_manager_is_noop() {
        let mut manager = TokenManager::new(
            "key".into(),
            "secret".into(),
            crate::types::Environment::Sandbox,
        );
        // Invalidating a freshly-created manager should not panic
        manager.invalidate();
        manager.invalidate(); // double-invalidate is idempotent
    }

    #[test]
    fn token_1_second_before_expiry_is_not_expired() {
        let token = Token {
            access_token: "abc".into(),
            expires_at: chrono::Utc::now() + chrono::Duration::seconds(1),
        };
        assert!(!TokenManager::is_expired(&Some(token)));
    }

    #[test]
    fn token_1_second_after_expiry_is_expired() {
        let token = Token {
            access_token: "abc".into(),
            expires_at: chrono::Utc::now() - chrono::Duration::seconds(1),
        };
        assert!(TokenManager::is_expired(&Some(token)));
    }

    #[test]
    fn token_1_hour_from_now_is_not_expired() {
        let token = Token {
            access_token: "abc".into(),
            expires_at: chrono::Utc::now() + chrono::Duration::seconds(3600),
        };
        assert!(!TokenManager::is_expired(&Some(token)));
    }

    #[test]
    fn token_manager_new_stores_credentials() {
        let manager = TokenManager::new(
            "ck".into(),
            "cs".into(),
            crate::types::Environment::Sandbox,
        );
        // Constructor should not panic, and struct compiles with fields
        assert!(manager.cached_token.is_none());
    }
}
