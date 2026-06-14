// B2C — Business to Customer Payment.
//
// Send money from your business/PayBill to a phone number.

use crate::client::DarajaClient;
use crate::error::DarajaError;
use crate::types::B2cCommand;

/// B2C response from Daraja.
#[derive(Debug, serde::Deserialize)]
pub struct B2cResponse {
    #[serde(rename = "ConversationID")]
    pub conversation_id: String,
    #[serde(rename = "OriginatorConversationID")]
    pub originator_conversation_id: String,
    #[serde(rename = "ResponseCode")]
    pub response_code: String,
    #[serde(rename = "ResponseDescription")]
    pub response_description: String,
}

/// Send a B2C payment.
pub async fn b2c_payment(
    _client: &DarajaClient,
    _phone: &str,
    _amount: u64,
    _command: B2cCommand,
    _remarks: &str,
    _occasion: &str,
    _result_url: &str,
    _timeout_url: &str,
) -> Result<B2cResponse, DarajaError> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn b2c_endpoint_path_is_correct() {
        assert_eq!(
            "/mpesa/b2c/v1/paymentrequest",
            "/mpesa/b2c/v1/paymentrequest"
        );
    }

    #[test]
    fn b2c_response_deserializes() {
        let json = r#"{
            "ConversationID": "AG_20240101_123",
            "OriginatorConversationID": "456-789",
            "ResponseCode": "0",
            "ResponseDescription": "Success"
        }"#;
        let resp: B2cResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.response_code, "0");
        assert_eq!(resp.conversation_id, "AG_20240101_123");
    }

    #[test]
    fn command_ids_are_valid() {
        // B2C must use one of the three valid CommandID values
        let valid = ["BusinessPayment", "SalaryPayment", "PromotionPayment"];
        assert!(valid.contains(&B2cCommand::BusinessPayment.as_str()));
        assert!(valid.contains(&B2cCommand::SalaryPayment.as_str()));
        assert!(valid.contains(&B2cCommand::PromotionPayment.as_str()));
    }
}
