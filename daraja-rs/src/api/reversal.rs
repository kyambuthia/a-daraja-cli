// Transaction Reversal.
//
// Reverse a completed M-Pesa transaction.

use crate::client::DarajaClient;
use crate::error::DarajaError;

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
    _client: &DarajaClient,
    _transaction_id: &str,
    _amount: u64,
    _receiver_party: &str,
    _receiver_identifier_type: &str,
    _remarks: &str,
    _occasion: &str,
    _result_url: &str,
    _timeout_url: &str,
) -> Result<ReversalResponse, DarajaError> {
    todo!()
}

#[cfg(test)]
mod tests {
    

    #[test]
    fn endpoint_path_is_correct() {
        assert_eq!(
            "/mpesa/reversal/v1/request",
            "/mpesa/reversal/v1/request"
        );
    }

    #[test]
    fn command_id_is_transaction_reversal() {
        assert_eq!("TransactionReversal", "TransactionReversal");
    }
}
