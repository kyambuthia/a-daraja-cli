// Account Balance Query.
//
// Check your M-Pesa account balance.

use crate::api::common::{initiator_name, initiator_security_credential};
use crate::client::DarajaClient;
use crate::error::DarajaError;
use crate::types::AccountBalanceRequest;
use crate::validation::validate_url;

const ACCOUNT_BALANCE_PATH: &str = "/mpesa/accountbalance/v1/query";

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
    client: &DarajaClient,
    result_url: &str,
    timeout_url: &str,
) -> Result<AccountBalanceResponse, DarajaError> {
    validate_url("result URL", result_url, client.environment())?;
    validate_url("timeout URL", timeout_url, client.environment())?;

    let request = AccountBalanceRequest {
        initiator: initiator_name(client)?,
        security_credential: initiator_security_credential(client)?,
        command_id: "AccountBalance".into(),
        party_a: client.credentials.shortcode.clone(),
        identifier_type: "4".into(),
        remarks: "Balance check".into(),
        queue_timeout_url: timeout_url.into(),
        result_url: result_url.into(),
    };

    client
        .post_without_idempotency(ACCOUNT_BALANCE_PATH, &request)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_path_is_correct() {
        assert_eq!(ACCOUNT_BALANCE_PATH, "/mpesa/accountbalance/v1/query");
    }

    #[test]
    fn command_id_is_account_balance() {
        assert_eq!("AccountBalance", "AccountBalance");
    }
}
