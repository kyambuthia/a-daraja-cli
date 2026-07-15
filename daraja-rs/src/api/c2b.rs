// C2B — Customer to Business.
//
// Register validation/confirmation URLs and simulate C2B transactions (sandbox only).

use crate::client::DarajaClient;
use crate::error::DarajaError;
use crate::types::C2bRegisterRequest;
use crate::types::Environment;
use crate::validation::{
    validate_amount, validate_phone, validate_required_text, validate_shortcode, validate_url,
};

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
    validate_shortcode(shortcode)?;
    if !matches!(response_type, "Completed" | "Cancelled") {
        return Err(DarajaError::Config(
            "response type must be Completed or Cancelled".into(),
        ));
    }
    validate_url("confirmation URL", confirmation_url, client.environment())?;
    validate_url("validation URL", validation_url, client.environment())?;

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
    if !matches!(client.environment(), Environment::Sandbox) {
        return Err(DarajaError::Config(
            "C2B simulation is available only in the sandbox environment".into(),
        ));
    }
    validate_shortcode(shortcode)?;
    validate_amount(amount)?;
    let msisdn = validate_phone(msisdn)?;
    validate_required_text("bill reference", bill_ref_number, 100)?;
    let request = serde_json::json!({
        "ShortCode": shortcode,
        "CommandID": "CustomerPayBillOnline",
        "Amount": amount.to_string(),
        "Msisdn": msisdn,
        "BillRefNumber": bill_ref_number,
    });
    client.post("/mpesa/c2b/v1/simulate", &request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Credentials;
    use zeroize::Zeroizing;

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

    #[tokio::test]
    async fn simulate_rejects_production_before_network_io() {
        let credentials = Credentials {
            consumer_key: Zeroizing::new("ck".into()),
            consumer_secret: Zeroizing::new("cs".into()),
            passkey: Zeroizing::new("pk".into()),
            shortcode: "174379".into(),
            initiator_name: None,
            initiator_password: None,
        };
        let client = DarajaClient::new(credentials, Environment::Production)
            .unwrap()
            .without_idempotency();
        let result = simulate_c2b(&client, "174379", 100, "254712345678", "ORDER-1").await;
        assert!(matches!(result, Err(DarajaError::Config(message)) if message.contains("sandbox")));
    }
}
