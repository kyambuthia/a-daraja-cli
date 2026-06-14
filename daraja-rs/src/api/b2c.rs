// B2C — Business to Customer Payment.
//
// Send money from your business/PayBill to a phone number.

use crate::client::DarajaClient;
use crate::error::DarajaError;
use crate::types::{B2cCommand, B2cRequest};

const B2C_PAYMENT_PATH: &str = "/mpesa/b2c/v1/paymentrequest";

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

/// Placeholder SecurityCredential for testing request shape.
///
/// Production: RSA-encrypt the initiator password with the Safaricom public
/// certificate, then base64-encode the ciphertext.
fn security_credential_placeholder(initiator_password: &str) -> String {
    base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        initiator_password,
    )
}

/// Send a B2C payment.
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
    let creds = &client.credentials;

    let initiator_name = creds
        .initiator_name
        .as_ref()
        .ok_or_else(|| DarajaError::Config("initiator_name is not configured".into()))?;
    let initiator_password = creds
        .initiator_password
        .as_ref()
        .ok_or_else(|| DarajaError::Config("initiator_password is not configured".into()))?;

    let request = B2cRequest {
        initiator_name: initiator_name.to_string(),
        security_credential: security_credential_placeholder(initiator_password),
        command_id: command.as_str().into(),
        amount: amount.to_string(),
        party_a: creds.shortcode.clone(),
        party_b: phone.into(),
        remarks: remarks.into(),
        queue_timeout_url: timeout_url.into(),
        result_url: result_url.into(),
        occasion: occasion.into(),
    };

    client.post(B2C_PAYMENT_PATH, &request).await
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
        // B2C must use one of the three valid CommandID values
        let valid = ["BusinessPayment", "SalaryPayment", "PromotionPayment"];
        assert!(valid.contains(&B2cCommand::BusinessPayment.as_str()));
        assert!(valid.contains(&B2cCommand::SalaryPayment.as_str()));
        assert!(valid.contains(&B2cCommand::PromotionPayment.as_str()));
    }

    #[test]
    fn security_credential_placeholder_base64_encodes_password() {
        let encoded = security_credential_placeholder("Safaricom999!*!");
        let decoded = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &encoded,
        )
        .unwrap();
        assert_eq!(String::from_utf8(decoded).unwrap(), "Safaricom999!*!");
    }

    // ── Async: integration tests ─────────────────────────────

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
            let server = MockServer::start().await;

            Mock::given(method("GET"))
                .and(path("/oauth/v1/generate"))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"access_token": "tok", "expires_in": "3599"})
                ))
                .mount(&server)
                .await;

            Mock::given(method("POST"))
                .and(path(B2C_PAYMENT_PATH))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({
                        "ConversationID": "AG_20240101_123",
                        "OriginatorConversationID": "456-789",
                        "ResponseCode": "0",
                        "ResponseDescription": "Success"
                    })
                ))
                .expect(1)
                .mount(&server)
                .await;

            let client = DarajaClient::new(test_credentials(), Environment::Sandbox)
                .unwrap()
                .with_base_url(server.uri());

            let resp = b2c_payment(
                &client,
                "254712345678",
                500,
                B2cCommand::BusinessPayment,
                "Salary",
                "June",
                "https://example.com/result",
                "https://example.com/timeout",
            )
            .await
            .unwrap();

            assert_eq!(resp.response_code, "0");
            assert_eq!(resp.conversation_id, "AG_20240101_123");
        }
    }
}