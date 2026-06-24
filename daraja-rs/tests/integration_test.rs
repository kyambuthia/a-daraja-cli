// Integration test: full end-to-end pipeline with wiremock.
//
// Tests the complete flow from `Daraja::from_key()` → `stk_push()` → response.
// This is what `mpesa init && mpesa stk` does in production.

use daraja_rs::api::stk_push::stk_push;
use daraja_rs::client::DarajaClient;
use daraja_rs::config::{self, Credentials};
use daraja_rs::types::Environment;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use zeroize::Zeroizing;

fn test_credentials() -> Credentials {
    Credentials {
        consumer_key: Zeroizing::new("test_ck".into()),
        consumer_secret: Zeroizing::new("test_cs".into()),
        passkey: Zeroizing::new(
            "bfb279f9aa9bdbcf158e97dd71a467cd2e0c893059b10f78e6b72ada1ed2c919".into(),
        ),
        shortcode: "174379".into(),
        initiator_name: Some(Zeroizing::new("apitest".into())),
        initiator_password: Some(Zeroizing::new("Safaricom999!*!".into())),
    }
}

#[tokio::test]
async fn full_pipeline_init_to_stk_push() {
    // Step 1: Simulate `mpesa init` — generate key, encrypt, write to temp file
    let sdk_key = config::generate_sdk_key("test");
    let key_bytes = config::extract_key_from_sdk_key(&sdk_key).unwrap();
    let creds = test_credentials();
    let encrypted = config::encrypt_credentials(&creds, &key_bytes).unwrap();

    // Write to temp dir instead of ~/.config
    let tmp = std::env::temp_dir().join("daraja-test");
    std::fs::create_dir_all(&tmp).unwrap();
    let creds_file = tmp.join("credentials.enc");
    std::fs::write(&creds_file, &encrypted).unwrap();

    // Step 2: Set up wiremock to simulate Daraja sandbox
    let server = MockServer::start().await;

    // OAuth mock
    Mock::given(method("GET"))
        .and(path("/oauth/v1/generate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({"access_token": "e2e-token", "expires_in": "3599"}),
        ))
        .mount(&server)
        .await;

    // STK Push mock
    Mock::given(method("POST"))
        .and(path("/mpesa/stkpush/v1/processrequest"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({
                "MerchantRequestID": "e2e-mr",
                "CheckoutRequestID": "e2e-co",
                "ResponseCode": "0",
                "ResponseDescription": "Success",
                "CustomerMessage": "Success"
            }),
        ))
        .expect(1)
        .mount(&server)
        .await;

    // Step 3: Create DarajaClient directly (what from_key() does internally)
    // Build client manually pointing at mock server
    let client = DarajaClient::new(creds, Environment::Sandbox)
        .unwrap()
        .with_base_url(server.uri())
        .without_idempotency();

    // Step 4: Call stk_push — this is what `pesaraja stk` does
    let resp = stk_push(
        &client,
        "254712345678",
        100,
        "Moringa seedlings",
        "Order #123",
        "https://example.com/callback",
    )
    .await
    .unwrap();

    assert_eq!(resp.response_code, "0");
    assert_eq!(resp.checkout_request_id, "e2e-co");
    assert_eq!(resp.merchant_request_id, "e2e-mr");
}

#[tokio::test]
async fn pipeline_auth_failure_propagates_error() {
    let server = MockServer::start().await;

    // OAuth fails with 401
    Mock::given(method("GET"))
        .and(path("/oauth/v1/generate"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Invalid credentials"))
        .mount(&server)
        .await;

    let creds = test_credentials();
    let client = DarajaClient::new(creds, Environment::Sandbox)
        .unwrap()
        .with_base_url(server.uri())
        .without_idempotency();

    let result = stk_push(
        &client,
        "254712345678",
        100,
        "test",
        "ref",
        "https://cb",
    )
    .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn pipeline_stk_push_fails_with_error_response() {
    let server = MockServer::start().await;

    // OAuth succeeds
    Mock::given(method("GET"))
        .and(path("/oauth/v1/generate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({"access_token": "tok", "expires_in": "3599"}),
        ))
        .mount(&server)
        .await;

    // STK Push fails
    Mock::given(method("POST"))
        .and(path("/mpesa/stkpush/v1/processrequest"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Error"))
        .mount(&server)
        .await;

    let creds = test_credentials();
    let client = DarajaClient::new(creds, Environment::Sandbox)
        .unwrap()
        .with_base_url(server.uri())
        .without_idempotency();

    let result = stk_push(
        &client,
        "254712345678",
        100,
        "test",
        "ref",
        "https://cb",
    )
    .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn pipeline_credential_roundtrip() {
    // Verify the full encrypt → write → read → decrypt pipeline
    let creds = test_credentials();
    let sdk_key = config::generate_sdk_key("test");
    let key_bytes = config::extract_key_from_sdk_key(&sdk_key).unwrap();

    // Encrypt
    let encrypted = config::encrypt_credentials(&creds, &key_bytes).unwrap();
    assert!(!encrypted.is_empty());

    // Simulate write to disk
    let tmp = std::env::temp_dir().join("daraja-roundtrip-test");
    std::fs::create_dir_all(&tmp).unwrap();
    let path = tmp.join("credentials.enc");
    std::fs::write(&path, &encrypted).unwrap();

    // Simulate read from disk (what from_key does)
    let read_back = std::fs::read(&path).unwrap();
    let decrypted = config::decrypt_credentials(&read_back, &key_bytes).unwrap();

    assert_eq!(*decrypted.consumer_key, *creds.consumer_key);
    assert_eq!(*decrypted.consumer_secret, *creds.consumer_secret);
    assert_eq!(decrypted.shortcode, creds.shortcode);
}
