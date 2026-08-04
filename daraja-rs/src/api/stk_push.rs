// STK Push — Lipa Na M-Pesa Online.
//
// Pushes a payment prompt to a customer's phone.

use crate::client::{DarajaClient, RequestOptions};
use crate::error::DarajaError;
use crate::payments::{Payment, PaymentStore, ReconciliationOutcome};
use crate::security::{current_timestamp, generate_password};
use crate::types::{StkPushRequest, StkPushResponse, StkQueryResponse, StkTransactionType};
use crate::validation::{validate_amount, validate_phone, validate_required_text, validate_url};
use serde::Serialize;

/// Ergonomic STK Push request options.
#[derive(Debug, Clone)]
pub struct StkPushOptions {
    pub phone: String,
    pub amount: u64,
    pub description: String,
    pub account_reference: String,
    pub callback_url: String,
    pub transaction_type: StkTransactionType,
}

impl StkPushOptions {
    pub fn new(phone: impl Into<String>, amount: u64, callback_url: impl Into<String>) -> Self {
        Self {
            phone: phone.into(),
            amount,
            description: "Payment".into(),
            account_reference: "Payment".into(),
            callback_url: callback_url.into(),
            transaction_type: StkTransactionType::PayBill,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn with_reference(mut self, account_reference: impl Into<String>) -> Self {
        self.account_reference = account_reference.into();
        self
    }

    pub fn with_transaction_type(mut self, transaction_type: StkTransactionType) -> Self {
        self.transaction_type = transaction_type;
        self
    }
}

/// Initiate an STK Push from typed request options.
pub async fn stk_push_with_options(
    client: &DarajaClient,
    options: &StkPushOptions,
) -> Result<StkPushResponse, DarajaError> {
    send_stk_push(
        client,
        &options.phone,
        options.amount,
        &options.description,
        &options.account_reference,
        &options.callback_url,
        options.transaction_type,
        RequestOptions::default(),
    )
    .await
}

/// Initiate an STK Push to a customer's phone.
///
/// `phone` may be in `+254...`, `07...`, `011...`, or `254...` format.
/// `amount` is in KES (whole numbers only).
/// `description` maps to `TransactionDesc` in the Daraja request.
/// `account_reference` is shown to the customer as the payment reference.
/// `callback_url` receives the STK Push result from Daraja.
#[derive(Debug, Clone, Serialize)]
struct StkPushQueryRequest {
    #[serde(rename = "BusinessShortCode")]
    business_short_code: String,
    #[serde(rename = "Password")]
    password: String,
    #[serde(rename = "Timestamp")]
    timestamp: String,
    #[serde(rename = "CheckoutRequestID")]
    checkout_request_id: String,
}

pub async fn stk_push(
    client: &DarajaClient,
    phone: &str,
    amount: u64,
    description: &str,
    account_reference: &str,
    callback_url: &str,
) -> Result<StkPushResponse, DarajaError> {
    stk_push_with_type(
        client,
        phone,
        amount,
        description,
        account_reference,
        callback_url,
        StkTransactionType::PayBill,
    )
    .await
}

/// Initiate an STK Push with an explicit transaction type (PayBill or Till).
pub async fn stk_push_with_type(
    client: &DarajaClient,
    phone: &str,
    amount: u64,
    description: &str,
    account_reference: &str,
    callback_url: &str,
    transaction_type: StkTransactionType,
) -> Result<StkPushResponse, DarajaError> {
    send_stk_push(
        client,
        phone,
        amount,
        description,
        account_reference,
        callback_url,
        transaction_type,
        RequestOptions::default(),
    )
    .await
}

/// Initiate an STK Push using a stable business-operation idempotency key.
#[allow(clippy::too_many_arguments)]
pub async fn stk_push_with_type_and_idempotency_key(
    client: &DarajaClient,
    phone: &str,
    amount: u64,
    description: &str,
    account_reference: &str,
    callback_url: &str,
    transaction_type: StkTransactionType,
    idempotency_key: &str,
) -> Result<StkPushResponse, DarajaError> {
    send_stk_push(
        client,
        phone,
        amount,
        description,
        account_reference,
        callback_url,
        transaction_type,
        RequestOptions::with_idempotency_key(idempotency_key),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn send_stk_push(
    client: &DarajaClient,
    phone: &str,
    amount: u64,
    description: &str,
    account_reference: &str,
    callback_url: &str,
    transaction_type: StkTransactionType,
    request_options: RequestOptions,
) -> Result<StkPushResponse, DarajaError> {
    validate_amount(amount)?;
    let phone = validate_phone(phone)?;
    validate_required_text("description", description, 100)?;
    validate_required_text("account reference", account_reference, 100)?;
    validate_url("callback URL", callback_url, client.environment())?;

    let shortcode = &client.credentials.shortcode;
    let passkey = client.credentials.passkey.as_str();
    let timestamp = current_timestamp();
    let password = generate_password(shortcode, passkey, &timestamp);

    let request = StkPushRequest {
        business_short_code: shortcode.clone(),
        password,
        timestamp,
        transaction_type: transaction_type.as_str().into(),
        amount: amount.to_string(),
        party_a: phone.clone(),
        party_b: shortcode.clone(),
        phone_number: phone,
        callback_url: callback_url.into(),
        account_reference: account_reference.into(),
        transaction_desc: description.into(),
    };

    client
        .post_with_options(
            "/mpesa/stkpush/v1/processrequest",
            &request,
            request_options,
        )
        .await
}

/// Query the status of an STK Push.
pub async fn stk_push_query(
    client: &DarajaClient,
    checkout_request_id: &str,
) -> Result<StkQueryResponse, DarajaError> {
    validate_required_text("checkout request ID", checkout_request_id, 128)?;
    let shortcode = &client.credentials.shortcode;
    let passkey = client.credentials.passkey.as_str();
    let timestamp = current_timestamp();
    let password = generate_password(shortcode, passkey, &timestamp);

    let request = StkPushQueryRequest {
        business_short_code: shortcode.clone(),
        password,
        timestamp,
        checkout_request_id: checkout_request_id.into(),
    };

    client
        .post_without_idempotency("/mpesa/stkpush/v1/query", &request)
        .await
}

/// Reconcile a stored payment through the authoritative STK query endpoint.
///
/// Network failures leave the payment unchanged so callers can retry the
/// reconciliation operation. A missing provider result code is treated as
/// pending rather than success.
pub async fn reconcile_stk_payment(
    client: &DarajaClient,
    store: &mut PaymentStore,
    payment_id: &str,
) -> Result<Payment, DarajaError> {
    let payment = store
        .get(payment_id)?
        .ok_or_else(|| DarajaError::TransactionNotFound(payment_id.into()))?;
    let checkout_request_id = payment.checkout_request_id.as_deref().ok_or_else(|| {
        DarajaError::Config("payment has no checkout request ID to reconcile".into())
    })?;
    let response = stk_push_query(client, checkout_request_id).await?;
    let outcome = match query_result_code(&response.result_code) {
        Some(0) => ReconciliationOutcome::Succeeded {
            transaction_id: None,
            receipt_number: None,
            detail: response
                .result_description
                .clone()
                .unwrap_or_else(|| "provider status query succeeded".into()),
        },
        Some(code) => ReconciliationOutcome::Failed {
            detail: response
                .result_description
                .clone()
                .unwrap_or_else(|| format!("provider result code {code}")),
        },
        None => ReconciliationOutcome::Pending {
            detail: response.response_description.clone(),
        },
    };
    store.apply_reconciliation_outcome(payment_id, outcome)
}

fn query_result_code(value: &Option<serde_json::Value>) -> Option<i64> {
    match value.as_ref()? {
        serde_json::Value::Number(number) => number.as_i64(),
        serde_json::Value::String(value) => value.parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{StkPushOptions, StkQueryResponse, query_result_code};
    use crate::types::StkTransactionType;

    #[test]
    fn stk_push_endpoint_path_is_correct() {
        assert_eq!(
            "/mpesa/stkpush/v1/processrequest",
            "/mpesa/stkpush/v1/processrequest"
        );
    }

    #[test]
    fn stk_push_query_endpoint_path_is_correct() {
        assert_eq!("/mpesa/stkpush/v1/query", "/mpesa/stkpush/v1/query");
    }

    #[test]
    fn phone_numbers_are_normalized_to_254_format() {
        fn normalize(phone: &str) -> String {
            let phone = phone.trim_start_matches('+');
            if let Some(local) = phone.strip_prefix('0') {
                format!("254{local}")
            } else {
                phone.to_string()
            }
        }
        assert_eq!(normalize("0712345678"), "254712345678");
        assert_eq!(normalize("+254712345678"), "254712345678");
        assert_eq!(normalize("254712345678"), "254712345678");
        assert_eq!(normalize("0112345678"), "254112345678");
    }

    #[test]
    fn amount_is_formatted_as_integer_string() {
        assert_eq!(100u64.to_string(), "100");
    }

    #[test]
    fn query_without_result_code_remains_pending() {
        let response = StkQueryResponse {
            merchant_request_id: "mr-1".into(),
            checkout_request_id: "ws_CO_1".into(),
            response_code: "0".into(),
            response_description: "Request accepted".into(),
            customer_message: None,
            result_code: None,
            result_description: None,
        };
        assert_eq!(query_result_code(&response.result_code), None);
    }

    #[test]
    fn query_result_code_accepts_string_and_number_forms() {
        assert_eq!(query_result_code(&Some(serde_json::json!("0"))), Some(0));
        assert_eq!(
            query_result_code(&Some(serde_json::json!(1032))),
            Some(1032)
        );
    }

    #[test]
    fn typed_options_provide_safe_defaults() {
        let options = StkPushOptions::new("254712345678", 100, "https://example.com/callback")
            .with_reference("ORDER-123")
            .with_description("Order payment")
            .with_transaction_type(StkTransactionType::Till);
        assert_eq!(options.account_reference, "ORDER-123");
        assert_eq!(options.description, "Order payment");
        assert_eq!(options.transaction_type, StkTransactionType::Till);
    }

    #[test]
    fn sandbox_shortcode_is_174379() {
        assert_eq!("174379", "174379");
    }

    // ── Async: integration tests ─────────────────────────────

    #[cfg(test)]
    mod async_tests {
        use super::super::stk_push;
        use crate::client::DarajaClient;
        use crate::config::Credentials;
        use crate::types::Environment;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        use zeroize::Zeroizing;

        fn test_credentials() -> Credentials {
            Credentials {
                consumer_key: Zeroizing::new("ck".into()),
                consumer_secret: Zeroizing::new("cs".into()),
                passkey: Zeroizing::new(
                    "bfb279f9aa9bdbcf158e97dd71a467cd2e0c893059b10f78e6b72ada1ed2c919".into(),
                ),
                shortcode: "174379".into(),
                initiator_name: Some(Zeroizing::new("apitest".into())),
                initiator_password: Some(Zeroizing::new("Safaricom999!*!".into())),
            }
        }

        #[tokio::test]
        async fn stk_push_sends_correct_request_body() {
            let server = MockServer::start().await;

            Mock::given(method("GET"))
                .and(path("/oauth/v1/generate"))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"access_token": "tok", "expires_in": "3599"}),
                ))
                .mount(&server)
                .await;

            Mock::given(method("POST"))
                .and(path("/mpesa/stkpush/v1/processrequest"))
                .and(wiremock::matchers::body_partial_json(serde_json::json!({
                    "BusinessShortCode": "174379",
                    "TransactionType": "CustomerPayBillOnline",
                    "Amount": "100",
                    "PartyA": "254712345678",
                    "PartyB": "174379",
                    "PhoneNumber": "254712345678",
                    "AccountReference": "Order #123",
                    "TransactionDesc": "Moringa seedlings"
                })))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "MerchantRequestID": "mr-1",
                    "CheckoutRequestID": "ws_CO_1",
                    "ResponseCode": "0",
                    "ResponseDescription": "Success",
                    "CustomerMessage": "Success"
                })))
                .expect(1)
                .mount(&server)
                .await;

            let client = DarajaClient::new(test_credentials(), Environment::Sandbox)
                .unwrap()
                .with_base_url(server.uri())
                .without_idempotency();
            let resp = stk_push(
                &client,
                "254712345678",
                100,
                "Moringa seedlings",
                "Order #123",
                "https://example.com/callback",
            )
            .await
            .unwrap();

            assert_eq!(resp.response_code, "0");
            assert_eq!(resp.checkout_request_id, "ws_CO_1");
        }

        #[tokio::test]
        async fn stk_push_handles_error_response() {
            let server = MockServer::start().await;

            Mock::given(method("GET"))
                .and(path("/oauth/v1/generate"))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"access_token": "tok", "expires_in": "3599"}),
                ))
                .mount(&server)
                .await;

            Mock::given(method("POST"))
                .and(path("/mpesa/stkpush/v1/processrequest"))
                .respond_with(ResponseTemplate::new(500).set_body_string("error"))
                .mount(&server)
                .await;

            let client = DarajaClient::new(test_credentials(), Environment::Sandbox)
                .unwrap()
                .with_base_url(server.uri())
                .without_idempotency();
            let result = stk_push(&client, "254712345678", 100, "test", "ref", "https://cb").await;
            assert!(result.is_err());
        }
    }
}
