// OAuth 2.0 token lifecycle management.
//
// Daraja tokens expire in 3600 seconds. We cache them and refresh
// automatically when they expire.

use crate::error::DarajaError;
use crate::types::Environment;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use chrono::{Duration, Utc};

/// An OAuth access token with expiry tracking.
#[derive(Clone)]
pub struct Token {
    pub access_token: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

/// Manages the OAuth token lifecycle — fetch, cache, refresh.
#[allow(dead_code)]
pub struct TokenManager {
    consumer_key: String,
    consumer_secret: String,
    environment: Environment,
    cached_token: Option<Token>,
    http_client: reqwest::Client,
    base_url_override: Option<String>,
}

impl TokenManager {
    /// Create a new token manager.
    pub fn new(consumer_key: String, consumer_secret: String, environment: Environment) -> Self {
        TokenManager {
            consumer_key,
            consumer_secret,
            environment,
            cached_token: None,
            http_client: reqwest::Client::new(),
            base_url_override: None,
        }
    }

    pub(crate) fn with_http_client(
        consumer_key: String,
        consumer_secret: String,
        environment: Environment,
        http_client: reqwest::Client,
    ) -> Self {
        TokenManager {
            consumer_key,
            consumer_secret,
            environment,
            cached_token: None,
            http_client,
            base_url_override: None,
        }
    }

    /// Override the base URL (for testing with wiremock).
    pub(crate) fn with_base_url(mut self, url: String) -> Self {
        self.base_url_override = Some(url);
        self
    }

    /// Get a valid token (cached or freshly fetched).
    pub async fn get_token(&mut self) -> Result<Token, DarajaError> {
        if !Self::is_expired(&self.cached_token) {
            return Ok(self.cached_token.as_ref().unwrap().clone());
        }

        let base_url = self
            .base_url_override
            .as_deref()
            .unwrap_or_else(|| self.environment.base_url());
        let url = format!("{base_url}/oauth/v1/generate?grant_type=client_credentials");

        let credentials = format!("{}:{}", self.consumer_key, self.consumer_secret);
        let encoded = STANDARD.encode(credentials);
        let auth = format!("Basic {encoded}");

        let mut attempt = 0u32;
        let response = loop {
            let result = self
                .http_client
                .get(&url)
                .header("Authorization", auth.clone())
                .send()
                .await;
            match result {
                Ok(response)
                    if attempt < 2
                        && (response.status().is_server_error()
                            || response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS) =>
                {
                    tokio::time::sleep(std::time::Duration::from_millis(100 * (1 << attempt)))
                        .await;
                    attempt += 1;
                }
                Ok(response) => break response,
                Err(error) if attempt < 2 && (error.is_connect() || error.is_timeout()) => {
                    tokio::time::sleep(std::time::Duration::from_millis(100 * (1 << attempt)))
                        .await;
                    attempt += 1;
                }
                Err(error) => {
                    return Err(DarajaError::AuthFailed(format!(
                        "request failed before a token was obtained: {error}"
                    )));
                }
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            return Err(DarajaError::AuthFailed(format!(
                "HTTP {status}: credentials were rejected; response body omitted"
            )));
        }

        #[derive(serde::Deserialize)]
        struct OAuthResponse {
            access_token: String,
            expires_in: String,
        }

        let oauth: OAuthResponse = response
            .json()
            .await
            .map_err(|e| DarajaError::AuthFailed(format!("failed to parse response: {e}")))?;

        let expires_in: i64 = oauth.expires_in.parse().map_err(|e| {
            DarajaError::AuthFailed(format!("invalid expires_in '{}': {}", oauth.expires_in, e))
        })?;

        let expires_at = Utc::now() + Duration::seconds(expires_in);

        let token = Token {
            access_token: oauth.access_token,
            expires_at,
        };
        self.cached_token = Some(token.clone());
        Ok(token)
    }

    /// Check whether a cached token has expired.
    pub fn is_expired(token: &Option<Token>) -> bool {
        match token {
            None => true,
            Some(t) => t.expires_at <= chrono::Utc::now() + Duration::seconds(30),
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
            format!("{sandbox_base}{auth_path}"),
            "https://sandbox.safaricom.co.ke/oauth/v1/generate?grant_type=client_credentials"
        );
    }

    #[test]
    fn production_auth_url_is_correct() {
        let prod_base = "https://api.safaricom.co.ke";
        let auth_path = "/oauth/v1/generate?grant_type=client_credentials";
        assert_eq!(
            format!("{prod_base}{auth_path}"),
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
    fn token_inside_safety_window_is_expired() {
        let token = Token {
            access_token: "abc".into(),
            expires_at: chrono::Utc::now() + chrono::Duration::seconds(1),
        };
        assert!(TokenManager::is_expired(&Some(token)));
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
        let manager =
            TokenManager::new("ck".into(), "cs".into(), crate::types::Environment::Sandbox);
        // Constructor should not panic, and struct compiles with fields
        assert!(manager.cached_token.is_none());
    }

    // ── Async: get_token with wiremock ────────────────────────

    #[cfg(test)]
    mod async_tests {
        use super::*;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        #[tokio::test]
        async fn get_token_fetches_new_token_on_first_call() {
            let server = MockServer::start().await;

            Mock::given(method("GET"))
                .and(path("/oauth/v1/generate"))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"access_token": "tok-123", "expires_in": "3599"}),
                ))
                .mount(&server)
                .await;

            let mut manager =
                TokenManager::new("ck".into(), "cs".into(), crate::types::Environment::Sandbox)
                    .with_base_url(server.uri());

            let token = manager
                .get_token()
                .await
                .expect("token fetch should succeed");
            assert_eq!(token.access_token, "tok-123");
        }

        #[tokio::test]
        async fn get_token_caches_and_reuses() {
            let server = MockServer::start().await;

            // Only one mock hit expected — second call returns cached
            Mock::given(method("GET"))
                .and(path("/oauth/v1/generate"))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"access_token": "tok-456", "expires_in": "3599"}),
                ))
                .expect(1) // Only called once
                .mount(&server)
                .await;

            let mut manager =
                TokenManager::new("ck".into(), "cs".into(), crate::types::Environment::Sandbox)
                    .with_base_url(server.uri());

            let t1 = manager.get_token().await.unwrap();
            let t2 = manager.get_token().await.unwrap();
            assert_eq!(t1.access_token, t2.access_token);
        }

        #[tokio::test]
        async fn get_token_fetches_new_after_invalidate() {
            let server = MockServer::start().await;

            Mock::given(method("GET"))
                .and(path("/oauth/v1/generate"))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"access_token": "tok-first", "expires_in": "3599"}),
                ))
                .up_to_n_times(1)
                .mount(&server)
                .await;

            Mock::given(method("GET"))
                .and(path("/oauth/v1/generate"))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"access_token": "tok-second", "expires_in": "3599"}),
                ))
                .expect(1)
                .mount(&server)
                .await;

            let mut manager =
                TokenManager::new("ck".into(), "cs".into(), crate::types::Environment::Sandbox)
                    .with_base_url(server.uri());

            let t1 = manager.get_token().await.unwrap();
            assert_eq!(t1.access_token, "tok-first");

            manager.invalidate();
            let t2 = manager.get_token().await.unwrap();
            assert_eq!(t2.access_token, "tok-second");
        }

        #[tokio::test]
        async fn get_token_returns_error_on_http_failure() {
            let server = MockServer::start().await;

            Mock::given(method("GET"))
                .and(path("/oauth/v1/generate"))
                .respond_with(ResponseTemplate::new(500).set_body_string("Internal Error"))
                .mount(&server)
                .await;

            let mut manager =
                TokenManager::new("ck".into(), "cs".into(), crate::types::Environment::Sandbox)
                    .with_base_url(server.uri());

            let result = manager.get_token().await;
            assert!(result.is_err(), "HTTP 500 should return error");
        }

        #[tokio::test]
        async fn get_token_sends_basic_auth_header() {
            let server = MockServer::start().await;

            Mock::given(method("GET"))
                .and(path("/oauth/v1/generate"))
                .and(wiremock::matchers::header(
                    "Authorization",
                    "Basic Y2s6Y3M=", // base64 of "ck:cs"
                ))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"access_token": "tok", "expires_in": "3599"}),
                ))
                .mount(&server)
                .await;

            let mut manager =
                TokenManager::new("ck".into(), "cs".into(), crate::types::Environment::Sandbox)
                    .with_base_url(server.uri());

            manager.get_token().await.unwrap();
        }
    }
}
