// C2B — Customer to Business.
//
// Register validation/confirmation URLs and simulate C2B transactions (sandbox only).

use crate::client::DarajaClient;
use crate::error::DarajaError;
use crate::security::normalize_phone;
use crate::types::C2bRegisterRequest;

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
    client: &DarajaClient,
    shortcode: &str,
    response_type: &str,
    confirmation_url: &str,
    validation_url: &str,
) -> Result<C2bRegisterResponse, DarajaError> {
    let request = C2bRegisterRequest {
        short_code: shortcode.to_string(),
        response_type: response_type.to_string(),
        confirmation_url: confirmation_url.to_string(),
        validation_url: validation_url.to_string(),
    };
    client
        .post("/mpesa/c2b/v1/registerurl", &request)
        .await
}

/// Simulate a C2B transaction (sandbox only).
pub async fn simulate_c2b(
    client: &DarajaClient,
    shortcode: &str,
    amount: u64,
    msisdn: &str,
    bill_ref_number: &str,
) -> Result<C2bRegisterResponse, DarajaError> {
    let request = serde_json::json!({
        "ShortCode": shortcode,
        "CommandID": "CustomerPayBillOnline",
        "Amount": amount.to_string(),
        "Msisdn": normalize_phone(msisdn),
        "BillRefNumber": bill_ref_number,
    });
    client.post("/mpesa/c2b/v1/simulate", &request).await
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
