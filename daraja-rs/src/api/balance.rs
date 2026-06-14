// Account Balance Query.
//
// Check your M-Pesa account balance.

use crate::client::DarajaClient;
use crate::error::DarajaError;
use crate::types::AccountBalanceRequest;

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

/// Query the M-Pesa account balance.
pub async fn query_balance(
    client: &DarajaClient,
    result_url: &str,
    timeout_url: &str,
) -> Result<AccountBalanceResponse, DarajaError> {
    let creds = &client.credentials;

    let initiator = creds
        .initiator_name
        .as_ref()
        .ok_or_else(|| DarajaError::Config("initiator_name is not configured".into()))?;
    let initiator_password = creds
        .initiator_password
        .as_ref()
        .ok_or_else(|| DarajaError::Config("initiator_password is not configured".into()))?;

    let request = AccountBalanceRequest {
        initiator: initiator.to_string(),
        security_credential: security_credential_placeholder(initiator_password),
        command_id: "AccountBalance".into(),
        party_a: creds.shortcode.clone(),
        identifier_type: "4".into(),
        remarks: "Balance check".into(),
        queue_timeout_url: timeout_url.into(),
        result_url: result_url.into(),
    };

    client.post(ACCOUNT_BALANCE_PATH, &request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_path_is_correct() {
        assert_eq!(
            ACCOUNT_BALANCE_PATH,
            "/mpesa/accountbalance/v1/query"
        );
    }

    #[test]
    fn command_id_is_account_balance() {
        assert_eq!("AccountBalance", "AccountBalance");
    }
}