// Shared types used across the SDK.
//
// Request/response structs, environment enum, transaction types.

use serde::{Deserialize, Serialize};

/// Daraja API environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Environment {
    Sandbox,
    Production,
}

impl Environment {
    /// Return the base URL for this environment.
    pub fn base_url(&self) -> &'static str {
        match self {
            Environment::Sandbox => "https://sandbox.safaricom.co.ke",
            Environment::Production => "https://api.safaricom.co.ke",
        }
    }
}

/// STK Push request body (Lipa Na M-Pesa Online).
#[derive(Debug, Clone, Serialize)]
pub struct StkPushRequest {
    #[serde(rename = "BusinessShortCode")]
    pub business_short_code: String,
    #[serde(rename = "Password")]
    pub password: String,
    #[serde(rename = "Timestamp")]
    pub timestamp: String,
    #[serde(rename = "TransactionType")]
    pub transaction_type: String,
    #[serde(rename = "Amount")]
    pub amount: String,
    #[serde(rename = "PartyA")]
    pub party_a: String,
    #[serde(rename = "PartyB")]
    pub party_b: String,
    #[serde(rename = "PhoneNumber")]
    pub phone_number: String,
    #[serde(rename = "CallBackURL")]
    pub callback_url: String,
    #[serde(rename = "AccountReference")]
    pub account_reference: String,
    #[serde(rename = "TransactionDesc")]
    pub transaction_desc: String,
}

/// STK Push response from Daraja.
#[derive(Debug, Clone, Deserialize)]
pub struct StkPushResponse {
    #[serde(rename = "MerchantRequestID")]
    pub merchant_request_id: String,
    #[serde(rename = "CheckoutRequestID")]
    pub checkout_request_id: String,
    #[serde(rename = "ResponseCode")]
    pub response_code: String,
    #[serde(rename = "ResponseDescription")]
    pub response_description: String,
    #[serde(rename = "CustomerMessage")]
    pub customer_message: String,
}

/// Response returned by the STK Push query endpoint.
///
/// Daraja uses the same envelope for initiating and querying prompts, but the
/// query response may additionally contain a result code and description.
#[derive(Debug, Clone, Deserialize)]
pub struct StkQueryResponse {
    #[serde(rename = "MerchantRequestID")]
    pub merchant_request_id: String,
    #[serde(rename = "CheckoutRequestID")]
    pub checkout_request_id: String,
    #[serde(rename = "ResponseCode")]
    pub response_code: String,
    #[serde(rename = "ResponseDescription")]
    pub response_description: String,
    #[serde(rename = "CustomerMessage", default)]
    pub customer_message: Option<String>,
    #[serde(rename = "ResultCode", default)]
    pub result_code: Option<serde_json::Value>,
    #[serde(rename = "ResultDesc", default)]
    pub result_description: Option<String>,
}

/// B2C payment request body.
#[derive(Debug, Clone, Serialize)]
pub struct B2cRequest {
    #[serde(rename = "InitiatorName")]
    pub initiator_name: String,
    #[serde(rename = "SecurityCredential")]
    pub security_credential: String,
    #[serde(rename = "CommandID")]
    pub command_id: String,
    #[serde(rename = "Amount")]
    pub amount: String,
    #[serde(rename = "PartyA")]
    pub party_a: String,
    #[serde(rename = "PartyB")]
    pub party_b: String,
    #[serde(rename = "Remarks")]
    pub remarks: String,
    #[serde(rename = "QueueTimeOutURL")]
    pub queue_timeout_url: String,
    #[serde(rename = "ResultURL")]
    pub result_url: String,
    #[serde(rename = "Occasion")]
    pub occasion: String,
}

/// Transaction status query request body.
#[derive(Debug, Clone, Serialize)]
pub struct TransactionStatusRequest {
    #[serde(rename = "Initiator")]
    pub initiator: String,
    #[serde(rename = "SecurityCredential")]
    pub security_credential: String,
    #[serde(rename = "CommandID")]
    pub command_id: String,
    #[serde(rename = "TransactionID")]
    pub transaction_id: String,
    #[serde(rename = "PartyA")]
    pub party_a: String,
    #[serde(rename = "IdentifierType")]
    pub identifier_type: String,
    #[serde(rename = "ResultURL")]
    pub result_url: String,
    #[serde(rename = "QueueTimeOutURL")]
    pub queue_timeout_url: String,
    #[serde(rename = "Remarks")]
    pub remarks: String,
    #[serde(rename = "Occasion")]
    pub occasion: String,
}

/// C2B Register URL request body.
#[derive(Debug, Clone, Serialize)]
pub struct C2bRegisterRequest {
    #[serde(rename = "ShortCode")]
    pub short_code: String,
    #[serde(rename = "ResponseType")]
    pub response_type: String,
    #[serde(rename = "ConfirmationURL")]
    pub confirmation_url: String,
    #[serde(rename = "ValidationURL")]
    pub validation_url: String,
}

/// Account balance query request body.
#[derive(Debug, Clone, Serialize)]
pub struct AccountBalanceRequest {
    #[serde(rename = "Initiator")]
    pub initiator: String,
    #[serde(rename = "SecurityCredential")]
    pub security_credential: String,
    #[serde(rename = "CommandID")]
    pub command_id: String,
    #[serde(rename = "PartyA")]
    pub party_a: String,
    #[serde(rename = "IdentifierType")]
    pub identifier_type: String,
    #[serde(rename = "Remarks")]
    pub remarks: String,
    #[serde(rename = "QueueTimeOutURL")]
    pub queue_timeout_url: String,
    #[serde(rename = "ResultURL")]
    pub result_url: String,
}

/// Reversal request body.
#[derive(Debug, Clone, Serialize)]
pub struct ReversalRequest {
    #[serde(rename = "Initiator")]
    pub initiator: String,
    #[serde(rename = "SecurityCredential")]
    pub security_credential: String,
    #[serde(rename = "CommandID")]
    pub command_id: String,
    #[serde(rename = "TransactionID")]
    pub transaction_id: String,
    #[serde(rename = "Amount")]
    pub amount: String,
    #[serde(rename = "ReceiverParty")]
    pub receiver_party: String,
    #[serde(rename = "RecieverIdentifierType")]
    pub receiver_identifier_type: String,
    #[serde(rename = "ResultURL")]
    pub result_url: String,
    #[serde(rename = "QueueTimeOutURL")]
    pub queue_timeout_url: String,
    #[serde(rename = "Remarks")]
    pub remarks: String,
    #[serde(rename = "Occasion")]
    pub occasion: String,
}

/// STK Push transaction type (PayBill vs Till).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StkTransactionType {
    /// PayBill — `CustomerPayBillOnline`
    PayBill,
    /// Till / Buy Goods — `CustomerBuyGoodsOnline`
    Till,
}

impl StkTransactionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            StkTransactionType::PayBill => "CustomerPayBillOnline",
            StkTransactionType::Till => "CustomerBuyGoodsOnline",
        }
    }
}

/// B2C CommandID values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum B2cCommand {
    BusinessPayment,
    SalaryPayment,
    PromotionPayment,
}

impl B2cCommand {
    pub fn as_str(&self) -> &'static str {
        match self {
            B2cCommand::BusinessPayment => "BusinessPayment",
            B2cCommand::SalaryPayment => "SalaryPayment",
            B2cCommand::PromotionPayment => "PromotionPayment",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_base_urls() {
        assert_eq!(
            Environment::Sandbox.base_url(),
            "https://sandbox.safaricom.co.ke"
        );
        assert_eq!(
            Environment::Production.base_url(),
            "https://api.safaricom.co.ke"
        );
    }

    #[test]
    fn stk_push_request_serializes_with_daraja_field_names() {
        let req = StkPushRequest {
            business_short_code: "174379".into(),
            password: "test".into(),
            timestamp: "20240101120000".into(),
            transaction_type: "CustomerPayBillOnline".into(),
            amount: "100".into(),
            party_a: "254712345678".into(),
            party_b: "174379".into(),
            phone_number: "254712345678".into(),
            callback_url: "https://example.com/callback".into(),
            account_reference: "Test".into(),
            transaction_desc: "Payment".into(),
        };

        let json = serde_json::to_string(&req).expect("serialization failed");
        assert!(json.contains("\"BusinessShortCode\""));
        assert!(json.contains("\"174379\""));
        assert!(json.contains("\"TransactionType\""));
        assert!(json.contains("\"CustomerPayBillOnline\""));
        assert!(json.contains("\"CallBackURL\""));
    }

    #[test]
    fn stk_push_response_deserializes_from_daraja_json() {
        let json = r#"{
            "MerchantRequestID": "123",
            "CheckoutRequestID": "ws_CO_456",
            "ResponseCode": "0",
            "ResponseDescription": "Success",
            "CustomerMessage": "Success"
        }"#;
        let resp: StkPushResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.response_code, "0");
        assert_eq!(resp.checkout_request_id, "ws_CO_456");
        assert_eq!(resp.customer_message, "Success");
    }

    #[test]
    fn b2c_command_ids_are_correct() {
        assert_eq!(B2cCommand::BusinessPayment.as_str(), "BusinessPayment");
        assert_eq!(B2cCommand::SalaryPayment.as_str(), "SalaryPayment");
        assert_eq!(B2cCommand::PromotionPayment.as_str(), "PromotionPayment");
    }

    #[test]
    fn b2c_request_serializes_correctly() {
        let req = B2cRequest {
            initiator_name: "apitest".into(),
            security_credential: "encrypted".into(),
            command_id: B2cCommand::BusinessPayment.as_str().into(),
            amount: "500".into(),
            party_a: "600000".into(),
            party_b: "254712345678".into(),
            remarks: "Salary".into(),
            queue_timeout_url: "https://example.com/timeout".into(),
            result_url: "https://example.com/result".into(),
            occasion: "June".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"InitiatorName\":\"apitest\""));
        assert!(json.contains("\"CommandID\":\"BusinessPayment\""));
        assert!(json.contains("\"Amount\":\"500\""));
    }

    #[test]
    fn transaction_status_request_serializes_correctly() {
        let req = TransactionStatusRequest {
            initiator: "apitest".into(),
            security_credential: "enc".into(),
            command_id: "TransactionStatusQuery".into(),
            transaction_id: "QEJ78H8JKR".into(),
            party_a: "600000".into(),
            identifier_type: "1".into(),
            result_url: "https://example.com/result".into(),
            queue_timeout_url: "https://example.com/timeout".into(),
            remarks: "Check".into(),
            occasion: "Verify".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"TransactionID\":\"QEJ78H8JKR\""));
        assert!(json.contains("\"IdentifierType\":\"1\""));
    }

    #[test]
    fn c2b_register_serializes_correctly() {
        let req = C2bRegisterRequest {
            short_code: "600000".into(),
            response_type: "Completed".into(),
            confirmation_url: "https://example.com/confirm".into(),
            validation_url: "https://example.com/validate".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"ConfirmationURL\""));
        assert!(json.contains("\"ValidationURL\""));
    }

    #[test]
    fn account_balance_request_serializes_correctly() {
        let req = AccountBalanceRequest {
            initiator: "apitest".into(),
            security_credential: "enc".into(),
            command_id: "AccountBalance".into(),
            party_a: "600000".into(),
            identifier_type: "4".into(),
            remarks: "Balance".into(),
            queue_timeout_url: "https://example.com/timeout".into(),
            result_url: "https://example.com/result".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"CommandID\":\"AccountBalance\""));
    }

    #[test]
    fn reversal_request_serializes_correctly() {
        let req = ReversalRequest {
            initiator: "apitest".into(),
            security_credential: "enc".into(),
            command_id: "TransactionReversal".into(),
            transaction_id: "QEJ78H8JKR".into(),
            amount: "100".into(),
            receiver_party: "600000".into(),
            receiver_identifier_type: "11".into(),
            result_url: "https://example.com/result".into(),
            queue_timeout_url: "https://example.com/timeout".into(),
            remarks: "Wrong".into(),
            occasion: "Refund".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"CommandID\":\"TransactionReversal\""));
        assert!(json.contains("\"RecieverIdentifierType\":\"11\""));
    }

    #[test]
    fn all_request_types_implement_debug_and_clone() {
        // Compile-time check: all types are Debug + Clone.
        fn assert_debug_clone<T: std::fmt::Debug + Clone>() {}

        assert_debug_clone::<StkPushRequest>();
        assert_debug_clone::<StkPushResponse>();
        assert_debug_clone::<B2cRequest>();
        assert_debug_clone::<TransactionStatusRequest>();
        assert_debug_clone::<C2bRegisterRequest>();
        assert_debug_clone::<AccountBalanceRequest>();
        assert_debug_clone::<ReversalRequest>();
        assert_debug_clone::<Environment>();
    }
}
