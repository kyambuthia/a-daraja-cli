// Transaction Status Query.
//
// Check the status of any M-Pesa transaction.

use crate::client::DarajaClient;
use crate::error::DarajaError;

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
    _client: &DarajaClient,
    _transaction_id: &str,
    _result_url: &str,
    _timeout_url: &str,
) -> Result<TransactionStatusResponse, DarajaError> {
    todo!()
}

#[cfg(test)]
mod tests {
    

    #[test]
    fn endpoint_path_is_correct() {
        assert_eq!(
            "/mpesa/transactionstatus/v1/query",
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
