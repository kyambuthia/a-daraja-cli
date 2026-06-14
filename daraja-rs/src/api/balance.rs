// Account Balance Query.
//
// Check your M-Pesa account balance.

use crate::client::DarajaClient;
use crate::error::DarajaError;

/// Account balance response.
#[derive(Debug, serde::Deserialize)]
pub struct AccountBalanceResponse {
    #[serde(rename = "OriginatorConversationID")]
    pub originator_conversation_id: String,
    #[serde(rename = "ConversationID")]
    pub conversation_id: String,
    #[serde(rename = "ResponseCode")]
    pub response_code: String,
    #[serde(rename = "ResponseDescription")]
    pub response_description: String,
}

/// Query the M-Pesa account balance.
pub async fn query_balance(
    _client: &DarajaClient,
    _result_url: &str,
    _timeout_url: &str,
) -> Result<AccountBalanceResponse, DarajaError> {
    todo!()
}

#[cfg(test)]
mod tests {
    

    #[test]
    fn endpoint_path_is_correct() {
        assert_eq!(
            "/mpesa/accountbalance/v1/query",
            "/mpesa/accountbalance/v1/query"
        );
    }

    #[test]
    fn command_id_is_account_balance() {
        assert_eq!("AccountBalance", "AccountBalance");
    }
}
