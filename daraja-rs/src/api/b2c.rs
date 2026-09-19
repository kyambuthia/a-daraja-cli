// B2C — Business to Customer Payment.
//
// Send money from your business/PayBill to a phone number.

use crate::api::common::initiator_security_credential;
use crate::client::{DarajaClient, RequestOptions};
use crate::error::DarajaError;
use crate::types::{B2cCommand, B2cRequest};
use crate::validation::{
    validate_amount, validate_optional_text, validate_phone, validate_required_text, validate_url,
};

const B2C_PAYMENT_PATH: &str = "/mpesa/b2c/v1/paymentrequest";

/// Ergonomic B2C payment request options.
#[derive(Debug, Clone)]
pub struct B2cPaymentOptions {
    pub phone: String,
    pub amount: u64,
    pub command: B2cCommand,
    pub remarks: String,
    pub occasion: String,
    pub result_url: String,
    pub timeout_url: String,
    pub idempotency_key: String,
}

impl B2cPaymentOptions {
    pub fn new(
        phone: impl Into<String>,
        amount: u64,
        result_url: impl Into<String>,
        timeout_url: impl Into<String>,
        idempotency_key: impl Into<String>,
    ) -> Self {
        Self {
            phone: phone.into(),
            amount,
            command: B2cCommand::BusinessPayment,
            remarks: "Payment".into(),
            occasion: String::new(),
            result_url: result_url.into(),
            timeout_url: timeout_url.into(),
            idempotency_key: idempotency_key.into(),
        }
    }

    pub fn with_command(mut self, command: B2cCommand) -> Self {
        self.command = command;
        self
    }

    pub fn with_remarks(mut self, remarks: impl Into<String>) -> Self {
        self.remarks = remarks.into();
        self
    }

    pub fn with_occasion(mut self, occasion: impl Into<String>) -> Self {
        self.occasion = occasion.into();
        self
    }
}

/// Send a B2C payment from typed request options.
pub async fn b2c_payment_with_options(
    client: &DarajaClient,
    options: &B2cPaymentOptions,
) -> Result<B2cResponse, DarajaError> {
    send_b2c_payment(
        client,
        &options.phone,
        options.amount,
        options.command,
        &options.remarks,
        &options.occasion,
        &options.result_url,
        &options.timeout_url,
        RequestOptions::with_idempotency_key(&options.idempotency_key),
    )
    .await
}

/// B2C response from Daraja.
#[derive(Debug, serde::Deserialize)]
pub struct B2cResponse {
    #[serde(rename = "ConversationID")]
    pub conversation_id: String,
    #[serde(rename = "OriginatorConversationID")]
    pub originator_conversation_id: String,
    #[serde(rename = "ResponseCode")]
    pub response_code: String,
    #[serde(rename = "ResponseDescription")]
    pub response_description: String,
}

/// Send a B2C payment.
#[allow(clippy::too_many_arguments)]
#[deprecated(note = "use b2c_payment_with_idempotency_key")]
pub async fn b2c_payment(
    client: &DarajaClient,
    phone: &str,
    amount: u64,
    command: B2cCommand,
    remarks: &str,
    occasion: &str,
    result_url: &str,
    timeout_url: &str,
) -> Result<B2cResponse, DarajaError> {
    let _ = (
        client,
        phone,
        amount,
        command,
        remarks,
        occasion,
        result_url,
        timeout_url,
    );
    Err(DarajaError::MissingIdempotencyKey)
}

/// Send a B2C payment using a stable business-operation idempotency key.
#[allow(clippy::too_many_arguments)]
pub async fn b2c_payment_with_idempotency_key(
    client: &DarajaClient,
    phone: &str,
    amount: u64,
    command: B2cCommand,
    remarks: &str,
    occasion: &str,
    result_url: &str,
    timeout_url: &str,
    idempotency_key: &str,
) -> Result<B2cResponse, DarajaError> {
    send_b2c_payment(
        client,
        phone,
        amount,
        command,
        remarks,
        occasion,
        result_url,
        timeout_url,
        RequestOptions::with_idempotency_key(idempotency_key),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn send_b2c_payment(
    client: &DarajaClient,
    phone: &str,
    amount: u64,
    command: B2cCommand,
    remarks: &str,
    occasion: &str,
    result_url: &str,
    timeout_url: &str,
    request_options: RequestOptions,
) -> Result<B2cResponse, DarajaError> {
    validate_amount(amount)?;
    let phone = validate_phone(phone)?;
    validate_required_text("remarks", remarks, 100)?;
    validate_optional_text("occasion", occasion, 100)?;
    validate_url("result URL", result_url, client.environment())?;
    validate_url("timeout URL", timeout_url, client.environment())?;

    let creds = &client.credentials;

    let initiator_name = creds
        .initiator_name
        .as_ref()
        .ok_or_else(|| DarajaError::Config("initiator_name is not configured".into()))?;

    let request = B2cRequest {
        initiator_name: initiator_name.to_string(),
        security_credential: initiator_security_credential(client)?,
        command_id: command.as_str().into(),
        amount: amount.to_string(),
        party_a: creds.shortcode.clone(),
        party_b: phone,
        remarks: remarks.into(),
        queue_timeout_url: timeout_url.into(),
        result_url: result_url.into(),
        occasion: occasion.into(),
    };

    client
        .post_with_options(B2C_PAYMENT_PATH, &request, request_options)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn b2c_endpoint_path_is_correct() {
        assert_eq!(B2C_PAYMENT_PATH, "/mpesa/b2c/v1/paymentrequest");
    }

    #[test]
    fn b2c_response_deserializes() {
        let json = r#"{
            "ConversationID": "AG_20240101_123",
            "OriginatorConversationID": "456-789",
            "ResponseCode": "0",
            "ResponseDescription": "Success"
        }"#;
        let resp: B2cResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.response_code, "0");
        assert_eq!(resp.conversation_id, "AG_20240101_123");
    }

    #[test]
    fn command_ids_are_valid() {
        let valid = ["BusinessPayment", "SalaryPayment", "PromotionPayment"];
        assert!(valid.contains(&B2cCommand::BusinessPayment.as_str()));
        assert!(valid.contains(&B2cCommand::SalaryPayment.as_str()));
        assert!(valid.contains(&B2cCommand::PromotionPayment.as_str()));
    }

    #[test]
    fn typed_options_support_fluent_payment_configuration() {
        let options = B2cPaymentOptions::new(
            "254712345678",
            100,
            "https://example.com/result",
            "https://example.com/timeout",
            "test:b2c:options",
        )
        .with_command(B2cCommand::SalaryPayment)
        .with_remarks("Payroll")
        .with_occasion("July");
        assert_eq!(options.command, B2cCommand::SalaryPayment);
        assert_eq!(options.remarks, "Payroll");
        assert_eq!(options.occasion, "July");
    }

    fn install_test_certificate() {
        use rand::rngs::OsRng;
        use rsa::{RsaPrivateKey, pkcs8::EncodePublicKey};

        let dir = tempfile::tempdir().expect("temp dir");
        let mut rng = OsRng;
        let private_key = RsaPrivateKey::new(&mut rng, 2048).expect("RSA key generation");
        let pem = private_key
            .to_public_key()
            .to_public_key_pem(rsa::pkcs8::LineEnding::LF)
            .expect("public key PEM encoding");
        let cert_path = dir.path().join("sandbox.cer");
        std::fs::write(&cert_path, pem).expect("write temp cert");
        std::mem::forget(dir);
        unsafe { std::env::set_var("DARAJA_CERT_PATH", cert_path) };
    }

    #[test]
    fn initiator_security_credential_uses_rsa_not_plain_base64() {
        use crate::config::Credentials;
        use crate::types::Environment;
        use zeroize::Zeroizing;

        install_test_certificate();

        let creds = Credentials {
            consumer_key: Zeroizing::new("ck".into()),
            consumer_secret: Zeroizing::new("cs".into()),
            passkey: Zeroizing::new("pk".into()),
            shortcode: "174379".into(),
            initiator_name: Some(Zeroizing::new("apitest".into())),
            initiator_password: Some(Zeroizing::new("Safaricom999!*!".into())),
        };
        let client = DarajaClient::new(creds, Environment::Sandbox)
            .unwrap()
            .dangerously_without_idempotency_for_testing();

        let credential = initiator_security_credential(&client).unwrap();
        let plain = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            "Safaricom999!*!",
        );
        assert_ne!(credential, plain);
    }

    #[cfg(test)]
    mod async_tests {
        use super::*;
        use crate::config::Credentials;
        use crate::types::Environment;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        use zeroize::Zeroizing;

        fn test_credentials() -> Credentials {
            Credentials {
                consumer_key: Zeroizing::new("ck".into()),
                consumer_secret: Zeroizing::new("cs".into()),
                passkey: Zeroizing::new("pk".into()),
                shortcode: "174379".into(),
                initiator_name: Some(Zeroizing::new("apitest".into())),
                initiator_password: Some(Zeroizing::new("Safaricom999!*!".into())),
            }
        }

        #[tokio::test]
        async fn b2c_payment_uses_correct_endpoint_path() {
            super::install_test_certificate();

            let server = MockServer::start().await;

            Mock::given(method("GET"))
                .and(path("/oauth/v1/generate"))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"access_token": "tok", "expires_in": "3599"}),
                ))
                .mount(&server)
                .await;

            Mock::given(method("POST"))
                .and(path(B2C_PAYMENT_PATH))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "ConversationID": "AG_20240101_123",
                    "OriginatorConversationID": "456-789",
                    "ResponseCode": "0",
                    "ResponseDescription": "Success"
                })))
                .expect(1)
                .mount(&server)
                .await;

            let client = DarajaClient::new(test_credentials(), Environment::Sandbox)
                .unwrap()
                .with_test_base_url(server.uri())
                .unwrap()
                .dangerously_without_idempotency_for_testing();

            let resp = b2c_payment_with_idempotency_key(
                &client,
                "254712345678",
                500,
                B2cCommand::BusinessPayment,
                "Salary",
                "June",
                "https://example.com/result",
                "https://example.com/timeout",
                "test:b2c:request-body",
            )
            .await
            .unwrap();

            assert_eq!(resp.response_code, "0");
            assert_eq!(resp.conversation_id, "AG_20240101_123");
        }
    }
}
