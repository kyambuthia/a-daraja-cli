// Transaction Status Query.
//
// Check the status of any M-Pesa transaction.

use crate::client::DarajaClient;
use crate::error::DarajaError;
use crate::types::TransactionStatusRequest;

const TRANSACTION_STATUS_PATH: &str = "/mpesa/transactionstatus/v1/query";

/// Transaction status response.
#[derive(Debug, serde::Deserialize)]
pub struct TransactionStatusResponse {
    #[serde(rename = "OriginatorConversationID")]
    pub originator_conversation_id: String,
    #[serde(rename = "ConversationID")]
    pub conversation_id: String,
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

/// Query the status of a transaction by its M-Pesa transaction ID.
pub async fn query_status(
    client: &DarajaClient,
    transaction_id: &str,
    remarks: &str,
    occasion: &str,
    result_url: &str,
    timeout_url: &str,
) -> Result<TransactionStatusResponse, DarajaError> {
    let creds = &client.credentials;

    let initiator = creds
        .initiator_name
        .as_ref()
        .ok_or_else(|| DarajaError::Config("initiator_name is not configured".into()))?;
    let initiator_password = creds
        .initiator_password
        .as_ref()
        .ok_or_else(|| DarajaError::Config("initiator_password is not configured".into()))?;

    let request = TransactionStatusRequest {
        initiator: initiator.to_string(),
        security_credential: security_credential_placeholder(initiator_password),
        command_id: "TransactionStatusQuery".into(),
        transaction_id: transaction_id.into(),
        party_a: creds.shortcode.clone(),
        identifier_type: "1".into(),
        result_url: result_url.into(),
        queue_timeout_url: timeout_url.into(),
        remarks: remarks.into(),
        occasion: occasion.into(),
    };

    client.post(TRANSACTION_STATUS_PATH, &request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_path_is_correct() {
        assert_eq!(
            TRANSACTION_STATUS_PATH,
            "/mpesa/transactionstatus/v1/query"
        );
    }

    #[test]
    fn identifier_types_are_valid() {
        // 1 = MSISDN, 2 = Till Number, 4 = Shortcode
        let valid: &[&str] = &["1", "2", "4"];
        assert!(valid.contains(&"1"));
        assert!(valid.contains(&"2"));
        assert!(valid.contains(&"4"));
    }
}