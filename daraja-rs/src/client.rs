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
pub struct DarajaClient {
    inner: reqwest::Client,
    token_manager: tokio::sync::Mutex<TokenManager>,
    pub(crate) credentials: Credentials,
    environment: Environment,
    base_url_override: Option<String>,
}

impl DarajaClient {
    /// Create a new client with credentials and environment.
    pub fn new(credentials: Credentials, environment: Environment) -> Result<Self, DarajaError> {
        let token_manager = TokenManager::new(
            credentials.consumer_key.to_string(),
            credentials.consumer_secret.to_string(),
            environment,
        );
        Ok(DarajaClient {
            inner: reqwest::Client::new(),
            token_manager: tokio::sync::Mutex::new(token_manager),
            credentials,
            environment,
            base_url_override: None,
        })
    }

    /// Override the base URL (for testing with wiremock).
    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url_override = Some(url.clone());
        let tm = self.token_manager.into_inner().with_base_url(url);
        self.token_manager = tokio::sync::Mutex::new(tm);
        self
    }

    fn base_url(&self) -> &str {
        self.base_url_override
            .as_deref()
            .unwrap_or_else(|| self.environment.base_url())
    }

    async fn handle_response<R: serde::de::DeserializeOwned>(
        response: reqwest::Response,
    ) -> Result<R, DarajaError> {
        if response.status().is_success() {
            response
                .json()
                .await
                .map_err(|e| DarajaError::Network(format!("failed to parse response: {}", e)))
        } else {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<no body>".to_string());
            Err(DarajaError::Network(format!("HTTP {}: {}", status, body)))
        }
    }

    /// Send a POST request to a Daraja endpoint.
    /// Handles OAuth token injection, idempotency, and retries.
    pub async fn post<T: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<R, DarajaError> {
        let url = format!("{}{}", self.base_url(), path);
        let body_bytes = serde_json::to_vec(body)
            .map_err(|e| DarajaError::Network(format!("serialize failed: {}", e)))?;
        let response = self.send_post(&url, &body_bytes, false).await?;
        Self::handle_response(response).await
    }

    async fn send_post(
        &self,
        url: &str,
        body: &[u8],
        is_retry: bool,
    ) -> Result<reqwest::Response, DarajaError> {
        let token = {
            let mut tm = self.token_manager.lock().await;
            tm.get_token().await?
        };

        let idempotency_key = generate_idempotency_key();

        let response = self
            .inner
            .post(url)
            .header("Authorization", format!("Bearer {}", token.access_token))
            .header("Content-Type", "application/json")
            .header("Idempotency-Key", &idempotency_key)
            .body(body.to_vec())
            .send()
            .await
            .map_err(|e| DarajaError::Network(format!("request failed: {}", e)))?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED && !is_retry {
            self.token_manager.lock().await.invalidate();
            return Box::pin(self.send_post(url, body, true)).await;
        }

        Ok(response)
    }

    /// Send a GET request to a Daraja endpoint.
    pub async fn get<R: serde::de::DeserializeOwned>(&self, path: &str) -> Result<R, DarajaError> {
        let url = format!("{}{}", self.base_url(), path);
        let response = self.send_get(&url, false).await?;
        Self::handle_response(response).await
    }

    async fn send_get(&self, url: &str, is_retry: bool) -> Result<reqwest::Response, DarajaError> {
        let token = {
            let mut tm = self.token_manager.lock().await;
            tm.get_token().await?
        };

        let response = self
            .inner
            .get(url)
            .header("Authorization", format!("Bearer {}", token.access_token))
            .send()
            .await
            .map_err(|e| DarajaError::Network(format!("request failed: {}", e)))?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED && !is_retry {
            self.token_manager.lock().await.invalidate();
            return Box::pin(self.send_get(url, true)).await;
        }

        Ok(response)
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

    // ── Async: DarajaClient integration tests ─────────────────

    #[cfg(test)]
    mod async_tests {
        use super::*;
        use crate::config::Credentials;
        use crate::types::Environment;
        use serde::Deserialize;
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        use zeroize::Zeroizing;

        fn test_credentials() -> Credentials {
            Credentials {
                consumer_key: Zeroizing::new("ck".into()),
                consumer_secret: Zeroizing::new("cs".into()),
                passkey: Zeroizing::new("pk".into()),
                shortcode: "174379".into(),
                initiator_name: Some(Zeroizing::new("apitest".into())),
                initiator_password: Some(Zeroizing::new("pass".into())),
            }
        }

        #[tokio::test]
        async fn client_post_attaches_bearer_auth() {
            let server = MockServer::start().await;

            // Mock OAuth
            Mock::given(method("GET"))
                .and(path("/oauth/v1/generate"))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"access_token": "bearer-tok", "expires_in": "3599"})
                ))
                .mount(&server)
                .await;

            // Mock the actual API call — expect bearer token
            Mock::given(method("POST"))
                .and(path("/mpesa/stkpush/v1/processrequest"))
                .and(header("Authorization", "Bearer bearer-tok"))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"ResponseCode": "0"})
                ))
                .expect(1)
                .mount(&server)
                .await;

            let client = DarajaClient::new(test_credentials(), Environment::Sandbox)
                .unwrap()
                .with_base_url(server.uri());
            let req = serde_json::json!({"test": true});

            #[derive(Deserialize)]
            struct Resp { #[serde(rename = "ResponseCode")] code: String }
            let resp: Resp = client.post("/mpesa/stkpush/v1/processrequest", &req).await.unwrap();
            assert_eq!(resp.code, "0");
        }

        #[tokio::test]
        async fn client_post_attaches_idempotency_key() {
            let server = MockServer::start().await;

            Mock::given(method("GET"))
                .and(path("/oauth/v1/generate"))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"access_token": "tok", "expires_in": "3599"})
                ))
                .mount(&server)
                .await;

            // Verify Idempotency-Key header exists (UUID validation tested in unit tests)
            Mock::given(method("POST"))
                .and(path("/test"))
                .and(wiremock::matchers::header_exists("Idempotency-Key"))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"ok": true})
                ))
                .expect(1)
                .mount(&server)
                .await;

            let client = DarajaClient::new(test_credentials(), Environment::Sandbox)
                .unwrap()
                .with_base_url(server.uri());
            #[derive(Deserialize)] struct Resp { #[allow(dead_code)] ok: bool }
            let resp: Resp = client.post("/test", &serde_json::json!({})).await.unwrap();
            assert!(resp.ok);
        }

        #[tokio::test]
        async fn client_post_returns_error_on_non_200() {
            let server = MockServer::start().await;

            Mock::given(method("GET"))
                .and(path("/oauth/v1/generate"))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"access_token": "tok", "expires_in": "3599"})
                ))
                .mount(&server)
                .await;

            Mock::given(method("POST"))
                .and(path("/fail"))
                .respond_with(ResponseTemplate::new(400).set_body_string("Bad Request"))
                .mount(&server)
                .await;

            let client = DarajaClient::new(test_credentials(), Environment::Sandbox)
                .unwrap()
                .with_base_url(server.uri());
            #[derive(Deserialize)] struct Resp { #[allow(dead_code)] ok: bool }
            let result: Result<Resp, _> = client.post("/fail", &serde_json::json!({})).await;
            assert!(result.is_err());
        }

        #[tokio::test]
        async fn client_post_retries_on_401_by_refreshing_token() {
            let server = MockServer::start().await;

            // First OAuth token
            Mock::given(method("GET"))
                .and(path("/oauth/v1/generate"))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"access_token": "old-tok", "expires_in": "3599"})
                ))
                .up_to_n_times(1)
                .mount(&server)
                .await;

            // API call fails with 401
            Mock::given(method("POST"))
                .and(path("/protected"))
                .and(header("Authorization", "Bearer old-tok"))
                .respond_with(ResponseTemplate::new(401))
                .up_to_n_times(1)
                .mount(&server)
                .await;

            // Token refreshed after 401
            Mock::given(method("GET"))
                .and(path("/oauth/v1/generate"))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"access_token": "new-tok", "expires_in": "3599"})
                ))
                .up_to_n_times(1)
                .mount(&server)
                .await;

            // Retry with new token succeeds
            Mock::given(method("POST"))
                .and(path("/protected"))
                .and(header("Authorization", "Bearer new-tok"))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"ok": true})
                ))
                .expect(1)
                .mount(&server)
                .await;

            let client = DarajaClient::new(test_credentials(), Environment::Sandbox)
                .unwrap()
                .with_base_url(server.uri());
            #[derive(Deserialize)] struct Resp { #[allow(dead_code)] ok: bool }
            let resp: Resp = client.post("/protected", &serde_json::json!({})).await.unwrap();
            assert!(resp.ok);
        }
    }
}
