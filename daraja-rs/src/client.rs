// HTTP client wrapper with OAuth, retry, and idempotency.
//
// Every outgoing request:
// 1. Ensures a valid OAuth token is cached
// 2. Attaches an Idempotency-Key header (UUID v4)
// 3. Tracks the key in a local cache to prevent double-sends
// 4. Refreshes authentication once on a definitive 401 response

use crate::auth::TokenManager;
use crate::config::Credentials;
use crate::error::DarajaError;
use crate::idempotency::{AcquireResult, IdempotencyStore};
use crate::types::Environment;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

/// The HTTP client that all API calls go through.
pub struct DarajaClient {
    inner: reqwest::Client,
    token_manager: tokio::sync::Mutex<TokenManager>,
    pub(crate) credentials: Credentials,
    environment: Environment,
    base_url_override: Option<String>,
    idempotency: Option<Mutex<IdempotencyStore>>,
}

/// Per-request controls for mutating Daraja operations.
#[derive(Debug, Clone, Default)]
pub struct RequestOptions {
    /// Stable business-operation key supplied by the caller.
    pub idempotency_key: Option<String>,
}

/// Network deadlines applied to OAuth and Daraja API calls.
#[derive(Debug, Clone, Copy)]
pub struct HttpConfig {
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
        }
    }
}

impl RequestOptions {
    pub fn with_idempotency_key(key: impl Into<String>) -> Self {
        Self {
            idempotency_key: Some(key.into()),
        }
    }
}

impl DarajaClient {
    /// Create a new client with credentials and environment.
    pub fn new(credentials: Credentials, environment: Environment) -> Result<Self, DarajaError> {
        Self::with_http_config(credentials, environment, HttpConfig::default())
    }

    /// Create a client with explicit connection and request deadlines.
    pub fn with_http_config(
        credentials: Credentials,
        environment: Environment,
        http_config: HttpConfig,
    ) -> Result<Self, DarajaError> {
        let inner = reqwest::Client::builder()
            .connect_timeout(http_config.connect_timeout)
            .timeout(http_config.request_timeout)
            .build()
            .map_err(|e| DarajaError::Config(format!("failed to configure HTTP client: {e}")))?;
        let token_manager = TokenManager::with_http_client(
            credentials.consumer_key.to_string(),
            credentials.consumer_secret.to_string(),
            environment,
            inner.clone(),
        );
        let idempotency = Some(Mutex::new(default_idempotency_store().map_err(|e| {
            DarajaError::Config(format!("failed to open idempotency cache: {e}"))
        })?));

        Ok(DarajaClient {
            inner,
            token_manager: tokio::sync::Mutex::new(token_manager),
            credentials,
            environment,
            base_url_override: None,
            idempotency,
        })
    }

    /// Disable idempotency caching (for tests).
    pub fn without_idempotency(mut self) -> Self {
        self.idempotency = None;
        self
    }

    /// Use a custom idempotency database path (for tests).
    pub fn with_idempotency_db(mut self, db_path: impl AsRef<Path>) -> Result<Self, DarajaError> {
        let store = IdempotencyStore::new(db_path.as_ref().to_str().ok_or_else(|| {
            DarajaError::Config("idempotency database path is not valid UTF-8".into())
        })?)
        .map_err(|e| DarajaError::Config(format!("failed to open idempotency cache: {e}")))?;
        self.idempotency = Some(Mutex::new(store));
        Ok(self)
    }

    /// Daraja environment this client is configured for.
    pub fn environment(&self) -> Environment {
        self.environment
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
            Err(api_error(status, &body))
        }
    }

    /// Send a POST request to a Daraja endpoint.
    /// Handles OAuth token injection, idempotency, and retries.
    pub async fn post<T: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<R, DarajaError> {
        self.post_with_options(path, body, RequestOptions::default())
            .await
    }

    /// Send a POST request with an optional stable, caller-supplied idempotency key.
    pub async fn post_with_options<T: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
        options: RequestOptions,
    ) -> Result<R, DarajaError> {
        let url = format!("{}{}", self.base_url(), path);
        let body_bytes = serde_json::to_vec(body)
            .map_err(|e| DarajaError::Network(format!("serialize failed: {}", e)))?;

        let fingerprint = request_fingerprint(&body_bytes);
        let caller_key = options
            .idempotency_key
            .unwrap_or_else(|| idempotency_key_for_body(&body_bytes));
        validate_idempotency_key(&caller_key)?;
        let scoped_key = scoped_idempotency_key(
            self.environment,
            &self.credentials.shortcode,
            path,
            &caller_key,
        );

        if let Some(cached) = self.check_idempotency_cache(&scoped_key, &fingerprint)? {
            return serde_json::from_slice(&cached).map_err(|e| {
                DarajaError::Network(format!("failed to parse cached response: {e}"))
            });
        }

        let response = match self
            .send_post(&url, &body_bytes, Some(&caller_key), false)
            .await
        {
            Ok(response) => response,
            Err(error) => {
                self.save_idempotency_unknown(&scoped_key);
                return Err(error);
            }
        };
        let status = response.status();
        let response_bytes = match response.bytes().await {
            Ok(bytes) => bytes,
            Err(error) => {
                self.save_idempotency_unknown(&scoped_key);
                return Err(DarajaError::Network(format!(
                    "failed to read response: {error}"
                )));
            }
        };

        if status.is_success() {
            let parsed = serde_json::from_slice(&response_bytes).map_err(|e| {
                self.save_idempotency_unknown(&scoped_key);
                DarajaError::Network(format!("failed to parse response: {e}"))
            })?;
            self.save_idempotency_success(&scoped_key, &response_bytes);
            Ok(parsed)
        } else {
            self.save_idempotency_failed(&scoped_key);
            let body = String::from_utf8_lossy(&response_bytes);
            Err(api_error(status, &body))
        }
    }

    /// Send a POST request without local idempotency caching.
    ///
    /// Daraja uses POST for status and balance queries as well as mutations.
    /// Use this method for requests whose response must be freshly evaluated.
    pub async fn post_without_idempotency<T: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<R, DarajaError> {
        let url = format!("{}{}", self.base_url(), path);
        let body_bytes = serde_json::to_vec(body)
            .map_err(|e| DarajaError::Network(format!("serialize failed: {}", e)))?;
        let response = self.send_post(&url, &body_bytes, None, false).await?;
        Self::handle_response(response).await
    }

    fn check_idempotency_cache(
        &self,
        scoped_key: &str,
        fingerprint: &str,
    ) -> Result<Option<Vec<u8>>, DarajaError> {
        let Some(store) = &self.idempotency else {
            return Ok(None);
        };

        let mut store = store
            .lock()
            .map_err(|_| DarajaError::Config("idempotency store lock poisoned".into()))?;
        let result = store
            .acquire(scoped_key, fingerprint)
            .map_err(|e| DarajaError::Config(format!("failed to reserve idempotency key: {e}")))?;

        match result {
            AcquireResult::Acquired => Ok(None),
            AcquireResult::Success(response) => Ok(Some(response)),
            AcquireResult::Pending => Err(DarajaError::IdempotencyConflict),
            AcquireResult::FingerprintMismatch => Err(DarajaError::IdempotencyKeyReused),
            AcquireResult::Failed => Err(DarajaError::IdempotencyPreviousFailed),
            AcquireResult::Unknown => Err(DarajaError::IdempotencyOutcomeUnknown),
        }
    }

    fn save_idempotency_success(&self, key: &str, response: &[u8]) {
        if let Some(store) = &self.idempotency
            && let Ok(store) = store.lock()
        {
            let _ = store.save_success(key, response);
        }
    }

    fn save_idempotency_failed(&self, key: &str) {
        if let Some(store) = &self.idempotency
            && let Ok(store) = store.lock()
        {
            let _ = store.save_failed(key);
        }
    }

    fn save_idempotency_unknown(&self, key: &str) {
        if let Some(store) = &self.idempotency
            && let Ok(store) = store.lock()
        {
            let _ = store.save_unknown(key);
        }
    }

    async fn send_post(
        &self,
        url: &str,
        body: &[u8],
        idempotency_key: Option<&str>,
        is_retry: bool,
    ) -> Result<reqwest::Response, DarajaError> {
        let token = {
            let mut tm = self.token_manager.lock().await;
            tm.get_token().await?
        };

        let mut request = self
            .inner
            .post(url)
            .header("Authorization", format!("Bearer {}", token.access_token))
            .header("Content-Type", "application/json");
        if let Some(idempotency_key) = idempotency_key {
            request = request.header("Idempotency-Key", idempotency_key);
        }
        let response = request
            .body(body.to_vec())
            .send()
            .await
            .map_err(|e| DarajaError::Network(format!("request failed: {}", e)))?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED && !is_retry {
            self.token_manager.lock().await.invalidate();
            return Box::pin(self.send_post(url, body, idempotency_key, true)).await;
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

fn default_idempotency_store() -> Result<IdempotencyStore, rusqlite::Error> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let dir = format!("{home}/.config/daraja");
    std::fs::create_dir_all(&dir).ok();
    IdempotencyStore::new(&format!("{dir}/idempotency.db"))
}

/// Derive a deterministic idempotency key from the serialized request body.
pub fn idempotency_key_for_body(body: &[u8]) -> String {
    let digest = Sha256::digest(body);
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    uuid::Uuid::from_bytes(bytes).to_string()
}

fn request_fingerprint(body: &[u8]) -> String {
    format!("{:x}", Sha256::digest(body))
}

fn scoped_idempotency_key(
    environment: Environment,
    shortcode: &str,
    path: &str,
    caller_key: &str,
) -> String {
    let environment = match environment {
        Environment::Sandbox => "sandbox",
        Environment::Production => "production",
    };
    request_fingerprint(format!("{environment}\0{shortcode}\0{path}\0{caller_key}").as_bytes())
}

fn validate_idempotency_key(key: &str) -> Result<(), DarajaError> {
    if key.is_empty()
        || key.len() > 128
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(DarajaError::Config(
            "idempotency key must be 1-128 ASCII letters, numbers, '.', ':', '_' or '-'".into(),
        ));
    }
    Ok(())
}

fn api_error(status: reqwest::StatusCode, body: &str) -> DarajaError {
    let parsed = serde_json::from_str::<serde_json::Value>(body).ok();
    let code = parsed
        .as_ref()
        .and_then(|value| {
            ["errorCode", "ResponseCode", "requestId"]
                .iter()
                .find_map(|key| value.get(key).and_then(|item| item.as_str()))
        })
        .unwrap_or("unknown")
        .chars()
        .take(64)
        .collect();
    let message = parsed
        .as_ref()
        .and_then(|value| {
            ["errorMessage", "ResponseDescription", "message"]
                .iter()
                .find_map(|key| value.get(key).and_then(|item| item.as_str()))
        })
        .unwrap_or("request rejected; response body omitted")
        .chars()
        .take(256)
        .collect();
    DarajaError::Api {
        status: status.as_u16(),
        code,
        message,
    }
}

// ── Idempotency ──────────────────────────────────────────────

use uuid::Uuid;

/// Generate a new random idempotency key (UUID v4).
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
        assert!(!is_valid_idempotency_key(
            "00000000-0000-0000-0000-000000000000"
        ));
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
        assert!(
            is_valid_idempotency_key(&key),
            "UUID parse is case-insensitive"
        );
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
                    serde_json::json!({"access_token": "bearer-tok", "expires_in": "3599"}),
                ))
                .mount(&server)
                .await;

            // Mock the actual API call — expect bearer token
            Mock::given(method("POST"))
                .and(path("/mpesa/stkpush/v1/processrequest"))
                .and(header("Authorization", "Bearer bearer-tok"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(serde_json::json!({"ResponseCode": "0"})),
                )
                .expect(1)
                .mount(&server)
                .await;

            let client = DarajaClient::new(test_credentials(), Environment::Sandbox)
                .unwrap()
                .with_base_url(server.uri())
                .without_idempotency();
            let req = serde_json::json!({"test": true});

            #[derive(Deserialize)]
            struct Resp {
                #[serde(rename = "ResponseCode")]
                code: String,
            }
            let resp: Resp = client
                .post("/mpesa/stkpush/v1/processrequest", &req)
                .await
                .unwrap();
            assert_eq!(resp.code, "0");
        }

        #[tokio::test]
        async fn client_post_attaches_idempotency_key() {
            let server = MockServer::start().await;

            Mock::given(method("GET"))
                .and(path("/oauth/v1/generate"))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"access_token": "tok", "expires_in": "3599"}),
                ))
                .mount(&server)
                .await;

            // Verify Idempotency-Key header exists (UUID validation tested in unit tests)
            Mock::given(method("POST"))
                .and(path("/test"))
                .and(wiremock::matchers::header_exists("Idempotency-Key"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})),
                )
                .expect(1)
                .mount(&server)
                .await;

            let client = DarajaClient::new(test_credentials(), Environment::Sandbox)
                .unwrap()
                .with_base_url(server.uri())
                .without_idempotency();
            #[derive(Deserialize)]
            struct Resp {
                #[allow(dead_code)]
                ok: bool,
            }
            let resp: Resp = client.post("/test", &serde_json::json!({})).await.unwrap();
            assert!(resp.ok);
        }

        #[tokio::test]
        async fn client_post_uses_caller_supplied_idempotency_key() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/oauth/v1/generate"))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"access_token": "tok", "expires_in": "3599"}),
                ))
                .mount(&server)
                .await;
            Mock::given(method("POST"))
                .and(path("/payment"))
                .and(header("Idempotency-Key", "order-123"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})),
                )
                .expect(1)
                .mount(&server)
                .await;

            let client = DarajaClient::new(test_credentials(), Environment::Sandbox)
                .unwrap()
                .with_base_url(server.uri())
                .without_idempotency();
            let _: serde_json::Value = client
                .post_with_options(
                    "/payment",
                    &serde_json::json!({"Amount": "100"}),
                    RequestOptions::with_idempotency_key("order-123"),
                )
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn caller_key_cannot_be_reused_for_different_payload() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/oauth/v1/generate"))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"access_token": "tok", "expires_in": "3599"}),
                ))
                .mount(&server)
                .await;
            Mock::given(method("POST"))
                .and(path("/payment"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})),
                )
                .expect(1)
                .mount(&server)
                .await;

            let dir = tempfile::tempdir().unwrap();
            let client = DarajaClient::new(test_credentials(), Environment::Sandbox)
                .unwrap()
                .with_base_url(server.uri())
                .with_idempotency_db(dir.path().join("idempotency.db"))
                .unwrap();
            let options = || RequestOptions::with_idempotency_key("order-123");
            let _: serde_json::Value = client
                .post_with_options("/payment", &serde_json::json!({"Amount": "100"}), options())
                .await
                .unwrap();
            let result: Result<serde_json::Value, _> = client
                .post_with_options("/payment", &serde_json::json!({"Amount": "200"}), options())
                .await;
            assert!(matches!(result, Err(DarajaError::IdempotencyKeyReused)));
        }

        #[tokio::test]
        async fn client_post_deduplicates_identical_requests() {
            let server = MockServer::start().await;

            Mock::given(method("GET"))
                .and(path("/oauth/v1/generate"))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"access_token": "tok", "expires_in": "3599"}),
                ))
                .mount(&server)
                .await;

            Mock::given(method("POST"))
                .and(path("/dedup"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(serde_json::json!({"ResponseCode": "0"})),
                )
                .expect(1)
                .mount(&server)
                .await;

            let dir = tempfile::tempdir().unwrap();
            let db_path = dir.path().join("idempotency.db");

            let client = DarajaClient::new(test_credentials(), Environment::Sandbox)
                .unwrap()
                .with_base_url(server.uri())
                .with_idempotency_db(&db_path)
                .unwrap();

            #[derive(Deserialize)]
            struct Resp {
                #[serde(rename = "ResponseCode")]
                code: String,
            }

            let body = serde_json::json!({"Amount": "100"});
            let first: Resp = client.post("/dedup", &body).await.unwrap();
            let second: Resp = client.post("/dedup", &body).await.unwrap();
            assert_eq!(first.code, "0");
            assert_eq!(second.code, "0");
        }

        #[tokio::test]
        async fn client_post_returns_error_on_non_200() {
            let server = MockServer::start().await;

            Mock::given(method("GET"))
                .and(path("/oauth/v1/generate"))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"access_token": "tok", "expires_in": "3599"}),
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
                .with_base_url(server.uri())
                .without_idempotency();
            #[derive(Deserialize)]
            struct Resp {
                #[allow(dead_code)]
                ok: bool,
            }
            let result: Result<Resp, _> = client.post("/fail", &serde_json::json!({})).await;
            assert!(result.is_err());
        }

        #[tokio::test]
        async fn api_errors_redact_unrecognized_response_fields() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/oauth/v1/generate"))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"access_token": "tok", "expires_in": "3599"}),
                ))
                .mount(&server)
                .await;
            Mock::given(method("POST"))
                .and(path("/fail"))
                .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                    "errorCode": "400.001",
                    "errorMessage": "Request rejected",
                    "consumerSecret": "must-not-leak"
                })))
                .mount(&server)
                .await;

            let client = DarajaClient::new(test_credentials(), Environment::Sandbox)
                .unwrap()
                .with_base_url(server.uri())
                .without_idempotency();
            let result: Result<serde_json::Value, _> =
                client.post("/fail", &serde_json::json!({})).await;
            let message = result.unwrap_err().to_string();
            assert!(message.contains("400.001"));
            assert!(message.contains("Request rejected"));
            assert!(!message.contains("must-not-leak"));
        }

        #[tokio::test]
        async fn client_enforces_request_timeout() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/oauth/v1/generate"))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"access_token": "tok", "expires_in": "3599"}),
                ))
                .mount(&server)
                .await;
            Mock::given(method("POST"))
                .and(path("/slow"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_delay(Duration::from_millis(200))
                        .set_body_json(serde_json::json!({"ok": true})),
                )
                .mount(&server)
                .await;

            let client = DarajaClient::with_http_config(
                test_credentials(),
                Environment::Sandbox,
                HttpConfig {
                    connect_timeout: Duration::from_millis(50),
                    request_timeout: Duration::from_millis(50),
                },
            )
            .unwrap()
            .with_base_url(server.uri())
            .without_idempotency();
            let result: Result<serde_json::Value, _> =
                client.post("/slow", &serde_json::json!({})).await;
            assert!(matches!(result, Err(DarajaError::Network(_))));
        }

        #[tokio::test]
        async fn client_post_retries_on_401_by_refreshing_token() {
            let server = MockServer::start().await;

            // First OAuth token
            Mock::given(method("GET"))
                .and(path("/oauth/v1/generate"))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"access_token": "old-tok", "expires_in": "3599"}),
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
                    serde_json::json!({"access_token": "new-tok", "expires_in": "3599"}),
                ))
                .up_to_n_times(1)
                .mount(&server)
                .await;

            // Retry with new token succeeds
            Mock::given(method("POST"))
                .and(path("/protected"))
                .and(header("Authorization", "Bearer new-tok"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})),
                )
                .expect(1)
                .mount(&server)
                .await;

            let client = DarajaClient::new(test_credentials(), Environment::Sandbox)
                .unwrap()
                .with_base_url(server.uri())
                .without_idempotency();
            #[derive(Deserialize)]
            struct Resp {
                #[allow(dead_code)]
                ok: bool,
            }
            let resp: Resp = client
                .post("/protected", &serde_json::json!({}))
                .await
                .unwrap();
            assert!(resp.ok);
        }
    }
}
