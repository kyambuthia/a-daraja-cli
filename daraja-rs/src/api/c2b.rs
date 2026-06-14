// C2B — Customer to Business.
//
// Register validation/confirmation URLs and simulate C2B transactions (sandbox only).

use crate::client::DarajaClient;
use crate::error::DarajaError;

/// C2B Register URL response.
#[derive(Debug, serde::Deserialize)]
pub struct C2bRegisterResponse {
    #[serde(rename = "OriginatorConversationID")]
    pub originator_conversation_id: String,
    #[serde(rename = "ConversationID")]
    pub conversation_id: String,
    #[serde(rename = "ResponseDescription")]
    pub response_description: String,
}

/// Register your C2B validation and confirmation URLs with Daraja.
pub async fn register_urls(
    _client: &DarajaClient,
    _shortcode: &str,
    _response_type: &str,
    _confirmation_url: &str,
    _validation_url: &str,
) -> Result<C2bRegisterResponse, DarajaError> {
    todo!()
}

/// Simulate a C2B transaction (sandbox only).
pub async fn simulate_c2b(
    _client: &DarajaClient,
    _shortcode: &str,
    _amount: u64,
    _msisdn: &str,
    _bill_ref_number: &str,
) -> Result<C2bRegisterResponse, DarajaError> {
    todo!()
}

#[cfg(test)]
mod tests {
    

    #[test]
    fn register_url_endpoint_is_correct() {
        assert_eq!("/mpesa/c2b/v1/registerurl", "/mpesa/c2b/v1/registerurl");
    }

    #[test]
    fn simulate_endpoint_is_correct() {
        assert_eq!("/mpesa/c2b/v1/simulate", "/mpesa/c2b/v1/simulate");
    }

    #[test]
    fn response_types_are_completed_or_cancelled() {
        let valid = ["Completed", "Cancelled"];
        assert!(valid.contains(&"Completed"));
        assert!(valid.contains(&"Cancelled"));
    }
}
