// Transaction Reversal.
//
// Reverse a completed M-Pesa transaction.

use crate::api::common::{initiator_name, initiator_security_credential};
use crate::client::DarajaClient;
use crate::error::DarajaError;
use crate::types::ReversalRequest;

const REVERSAL_PATH: &str = "/mpesa/reversal/v1/request";
const COMMAND_ID: &str = "TransactionReversal";

/// Reversal response.
#[derive(Debug, serde::Deserialize)]
pub struct ReversalResponse {
    #[serde(rename = "OriginatorConversationID")]
    pub originator_conversation_id: String,
    #[serde(rename = "ConversationID")]
    pub conversation_id: String,
    #[serde(rename = "ResponseCode")]
    pub response_code: String,
    #[serde(rename = "ResponseDescription")]
    pub response_description: String,
}

/// Reverse a completed transaction.
pub async fn reverse_transaction(
    client: &DarajaClient,
    transaction_id: &str,
    amount: u64,
    receiver_party: &str,
    receiver_identifier_type: &str,
    remarks: &str,
    occasion: &str,
    result_url: &str,
    timeout_url: &str,
) -> Result<ReversalResponse, DarajaError> {
    let request = ReversalRequest {
        initiator: initiator_name(client)?,
        security_credential: initiator_security_credential(client)?,
        command_id: COMMAND_ID.into(),
        transaction_id: transaction_id.into(),
        amount: amount.to_string(),
        receiver_party: receiver_party.into(),
        receiver_identifier_type: receiver_identifier_type.into(),
        result_url: result_url.into(),
        queue_timeout_url: timeout_url.into(),
        remarks: remarks.into(),
        occasion: occasion.into(),
    };

    client.post(REVERSAL_PATH, &request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_path_is_correct() {
        assert_eq!(REVERSAL_PATH, "/mpesa/reversal/v1/request");
    }

    #[test]
    fn command_id_is_transaction_reversal() {
        assert_eq!(COMMAND_ID, "TransactionReversal");
    }

    #[cfg(test)]
    mod async_tests {
        use super::super::{reverse_transaction, REVERSAL_PATH};
        use crate::client::DarajaClient;
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

        fn with_test_certificate() {
            use rand::rngs::OsRng;
            use rsa::{pkcs8::EncodePublicKey, RsaPrivateKey};

            let dir = tempfile::tempdir().expect("temp dir");
            let mut rng = OsRng;
            let private_key = RsaPrivateKey::new(&mut rng, 2048).expect("RSA key generation");
            let pem = private_key
                .to_public_key()
                .to_public_key_pem(rsa::pkcs8::LineEnding::LF)
                .expect("public key PEM encoding");
            let cert_path = dir.path().join("sandbox.cer");
            std::fs::write(&cert_path, pem).expect("write temp cert");
            // Leak tempdir so the cert file survives for the rest of the test process.
            std::mem::forget(dir);
            unsafe { std::env::set_var("DARAJA_CERT_PATH", cert_path) };
        }

        #[tokio::test]
        async fn reverse_transaction_sends_correct_request_body() {
            with_test_certificate();

            let server = MockServer::start().await;

            Mock::given(method("GET"))
                .and(path("/oauth/v1/generate"))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"access_token": "tok", "expires_in": "3599"})
                ))
                .mount(&server)
                .await;

            Mock::given(method("POST"))
                .and(path(REVERSAL_PATH))
                .and(wiremock::matchers::body_partial_json(
                    serde_json::json!({
                        "Initiator": "apitest",
                        "CommandID": "TransactionReversal",
                        "TransactionID": "QEJ78H8JKR",
                        "Amount": "100",
                        "ReceiverParty": "600000",
                        "RecieverIdentifierType": "11",
                        "ResultURL": "https://example.com/result",
                        "QueueTimeOutURL": "https://example.com/timeout",
                        "Remarks": "Wrong amount",
                        "Occasion": "Refund"
                    })
                ))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({
                        "OriginatorConversationID": "456-789",
                        "ConversationID": "AG_20240101_123",
                        "ResponseCode": "0",
                        "ResponseDescription": "Success"
                    })
                ))
                .expect(1)
                .mount(&server)
                .await;

            let client = DarajaClient::new(test_credentials(), Environment::Sandbox)
                .unwrap()
                .with_base_url(server.uri())
                .without_idempotency();

            let resp = reverse_transaction(
                &client,
                "QEJ78H8JKR",
                100,
                "600000",
                "11",
                "Wrong amount",
                "Refund",
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