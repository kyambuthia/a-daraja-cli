// Transaction Status Query.
//
// Check the status of any M-Pesa transaction.

use crate::api::common::{initiator_name, initiator_security_credential};
use crate::client::DarajaClient;
use crate::error::DarajaError;
use crate::types::TransactionStatusRequest;
use crate::validation::{validate_optional_text, validate_required_text, validate_url};

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

/// Query the status of a transaction by its M-Pesa transaction ID.
pub async fn query_status(
    client: &DarajaClient,
    transaction_id: &str,
    remarks: &str,
    occasion: &str,
    result_url: &str,
    timeout_url: &str,
) -> Result<TransactionStatusResponse, DarajaError> {
    validate_required_text("transaction ID", transaction_id, 128)?;
    validate_required_text("remarks", remarks, 100)?;
    validate_optional_text("occasion", occasion, 100)?;
    validate_url("result URL", result_url, client.environment())?;
    validate_url("timeout URL", timeout_url, client.environment())?;

    let request = TransactionStatusRequest {
        initiator: initiator_name(client)?,
        security_credential: initiator_security_credential(client)?,
        command_id: "TransactionStatusQuery".into(),
        transaction_id: transaction_id.into(),
        party_a: client.credentials.shortcode.clone(),
        identifier_type: "1".into(),
        result_url: result_url.into(),
        queue_timeout_url: timeout_url.into(),
        remarks: remarks.into(),
        occasion: occasion.into(),
    };

    client
        .post_without_idempotency(TRANSACTION_STATUS_PATH, &request)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_path_is_correct() {
        assert_eq!(TRANSACTION_STATUS_PATH, "/mpesa/transactionstatus/v1/query");
    }

    #[test]
    fn identifier_types_are_valid() {
        let valid: &[&str] = &["1", "2", "4"];
        assert!(valid.contains(&"1"));
        assert!(valid.contains(&"2"));
        assert!(valid.contains(&"4"));
    }
}
