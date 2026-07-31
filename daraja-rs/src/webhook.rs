// Callback / webhook verification.
//
// Cryptographic authentication is not available from the documented callback
// contract. Parsing and matching callbacks never makes them trusted.

use crate::error::DarajaError;
use crate::validation::{validate_phone, validate_required_text, validate_shortcode};

/// Application-owned callback authenticator.
///
/// Pesaraja does not provide a default implementation because the documented
/// Daraja callback contract does not define a cryptographic scheme. An
/// application may implement this trait using its own verified gateway or
/// documented provider integration.
pub trait CallbackAuthenticator {
    fn verify(&self, body: &[u8], signature_header: Option<&str>) -> Result<(), DarajaError>;
}

/// Verify that a C2B callback request came from Safaricom.
///
/// `body` — the raw request body bytes.
/// `signature_header` — a reserved authenticator value, if one is configured.
///
/// Returns an error until the configured Daraja cryptographic verification
/// scheme is available. Header presence is never treated as authentication.
pub fn verify_callback(body: &[u8], signature_header: Option<&str>) -> Result<(), DarajaError> {
    if body.is_empty() {
        return Err(DarajaError::InvalidCallbackSignature);
    }
    if signature_header.is_none_or(|value| value.trim().is_empty()) {
        return Err(DarajaError::InvalidCallbackSignature);
    }

    // Safaricom's current public documentation does not expose enough
    // information to implement the signature algorithm and key management
    // safely. Never treat header presence as authentication.
    Err(DarajaError::CallbackVerificationUnavailable)
}

/// Verify a callback with an application-supplied authenticator.
///
/// Structural parsing, callback matching, and this authenticator are separate
/// steps. Successful verification still requires replay protection and
/// authoritative transaction reconciliation before fulfilment.
pub fn verify_callback_with<A: CallbackAuthenticator>(
    body: &[u8],
    signature_header: Option<&str>,
    authenticator: &A,
) -> Result<(), DarajaError> {
    if body.is_empty() {
        return Err(DarajaError::InvalidCallbackSignature);
    }
    if signature_header.is_none_or(|value| value.trim().is_empty()) {
        return Err(DarajaError::InvalidCallbackSignature);
    }
    authenticator.verify(body, signature_header)
}

/// Verify that a C2B confirmation body contains the expected fields.
pub fn validate_c2b_confirmation(body: &[u8]) -> Result<C2bConfirmation, DarajaError> {
    let mut confirmation: C2bConfirmation = serde_json::from_slice(body)
        .map_err(|e| DarajaError::InvalidCallback(format!("malformed JSON: {e}")))?;
    validate_required_text("transaction type", &confirmation.transaction_type, 64)
        .map_err(callback_validation_error)?;
    validate_required_text("transaction ID", &confirmation.trans_id, 128)
        .map_err(callback_validation_error)?;
    if confirmation.trans_time.len() != 14
        || !confirmation
            .trans_time
            .bytes()
            .all(|byte| byte.is_ascii_digit())
    {
        return Err(DarajaError::InvalidCallback(
            "transaction time must contain 14 digits".into(),
        ));
    }
    if !is_positive_decimal(&confirmation.trans_amount) {
        return Err(DarajaError::InvalidCallback(
            "transaction amount must be positive".into(),
        ));
    }
    validate_shortcode(&confirmation.business_short_code).map_err(callback_validation_error)?;
    validate_required_text("bill reference", &confirmation.bill_ref_number, 100)
        .map_err(callback_validation_error)?;
    confirmation.msisdn =
        validate_phone(&confirmation.msisdn).map_err(callback_validation_error)?;
    Ok(confirmation)
}

fn callback_validation_error(error: DarajaError) -> DarajaError {
    DarajaError::InvalidCallback(error.to_string())
}

fn is_positive_decimal(value: &str) -> bool {
    let mut parts = value.split('.');
    let Some(whole) = parts.next() else {
        return false;
    };
    let fraction = parts.next();
    if parts.next().is_some()
        || whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || fraction
            .is_some_and(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return false;
    }
    value.bytes().any(|byte| matches!(byte, b'1'..=b'9'))
}

/// Stable key applications can use to reject C2B callback replay.
pub fn c2b_callback_deduplication_key(callback: &C2bConfirmation) -> String {
    format!("c2b:transaction:{}", callback.trans_id)
}

/// A parsed C2B confirmation callback.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct C2bConfirmation {
    #[serde(rename = "TransactionType")]
    pub transaction_type: String,
    #[serde(rename = "TransID")]
    pub trans_id: String,
    #[serde(rename = "TransTime")]
    pub trans_time: String,
    #[serde(rename = "TransAmount")]
    pub trans_amount: String,
    #[serde(rename = "BusinessShortCode")]
    pub business_short_code: String,
    #[serde(rename = "BillRefNumber")]
    pub bill_ref_number: String,
    #[serde(rename = "MSISDN")]
    pub msisdn: String,
    #[serde(rename = "FirstName")]
    pub first_name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AcceptingAuthenticator;

    impl CallbackAuthenticator for AcceptingAuthenticator {
        fn verify(&self, body: &[u8], signature_header: Option<&str>) -> Result<(), DarajaError> {
            if body == b"trusted" && signature_header == Some("valid") {
                Ok(())
            } else {
                Err(DarajaError::InvalidCallbackSignature)
            }
        }
    }

    #[test]
    fn missing_signature_header_is_rejected() {
        let body = br#"{"TransactionType":"Pay Bill"}"#;
        let result = verify_callback(body, None);
        assert!(result.is_err());
    }

    #[test]
    fn empty_signature_header_is_rejected() {
        let body = br#"{"TransactionType":"Pay Bill"}"#;
        let result = verify_callback(body, Some(""));
        assert!(result.is_err());
    }

    #[test]
    fn empty_body_is_rejected() {
        let result = verify_callback(&[], Some("some-signature"));
        assert!(result.is_err());
    }

    #[test]
    fn c2b_confirmation_parses_valid_json() {
        let json = r#"{
            "TransactionType": "Pay Bill",
            "TransID": "QEJ78H8JKR",
            "TransTime": "20240101120000",
            "TransAmount": "500.00",
            "BusinessShortCode": "174379",
            "BillRefNumber": "Test",
            "MSISDN": "254712345678",
            "FirstName": "John"
        }"#;
        let conf: C2bConfirmation = serde_json::from_str(json).unwrap();
        assert_eq!(conf.trans_id, "QEJ78H8JKR");
        assert_eq!(conf.trans_amount, "500.00");
        assert_eq!(conf.msisdn, "254712345678");
        assert_eq!(conf.first_name.as_deref(), Some("John"));
    }

    #[test]
    fn c2b_confirmation_handles_missing_firstname() {
        let json = r#"{
            "TransactionType": "Pay Bill",
            "TransID": "AAA",
            "TransTime": "20240101120000",
            "TransAmount": "100.00",
            "BusinessShortCode": "174379",
            "BillRefNumber": "Test",
            "MSISDN": "254712345678"
        }"#;
        let conf: C2bConfirmation = serde_json::from_str(json).unwrap();
        assert_eq!(conf.first_name, None);
    }

    // ── Edge cases ───────────────────────────────────────────

    #[test]
    fn verify_callback_rejects_whitespace_only_body() {
        let result = verify_callback(b"   ", Some("sig"));
        assert!(matches!(
            result,
            Err(DarajaError::CallbackVerificationUnavailable)
        ));
    }

    #[test]
    fn verify_callback_rejects_whitespace_only_signature() {
        let result = verify_callback(b"body", Some("   "));
        assert!(matches!(result, Err(DarajaError::InvalidCallbackSignature)));
    }

    #[test]
    fn verify_callback_fails_closed_until_crypto_is_configured() {
        let result = verify_callback(b"{\"valid\":true}", Some("abc123signature"));
        assert!(matches!(
            result,
            Err(DarajaError::CallbackVerificationUnavailable)
        ));
    }

    #[test]
    fn application_authenticator_can_verify_callback() {
        let authenticator = AcceptingAuthenticator;
        assert!(verify_callback_with(b"trusted", Some("valid"), &authenticator).is_ok());
    }

    #[test]
    fn application_authenticator_cannot_bypass_basic_input_checks() {
        let authenticator = AcceptingAuthenticator;
        assert!(matches!(
            verify_callback_with(b"", Some("valid"), &authenticator),
            Err(DarajaError::InvalidCallbackSignature)
        ));
        assert!(matches!(
            verify_callback_with(b"trusted", None, &authenticator),
            Err(DarajaError::InvalidCallbackSignature)
        ));
    }

    #[test]
    fn validate_c2b_confirmation_accepts_valid_json() {
        let json = br#"{
            "TransactionType": "Pay Bill",
            "TransID": "QEJ78H8JKR",
            "TransTime": "20240101120000",
            "TransAmount": "500.00",
            "BusinessShortCode": "174379",
            "BillRefNumber": "Test",
            "MSISDN": "254712345678",
            "FirstName": "John"
        }"#;
        let result = validate_c2b_confirmation(json);
        assert!(result.is_ok());
    }

    #[test]
    fn shared_callback_fixture_matches_rust_contract() {
        let fixture = include_bytes!("../fixtures/callback-contract.json");
        let root: serde_json::Value = serde_json::from_slice(fixture).unwrap();
        let body = serde_json::to_vec(&root["c2b_confirmation"]).unwrap();
        let confirmation = validate_c2b_confirmation(&body).unwrap();
        assert_eq!(confirmation.bill_ref_number, "ORDER-123");
    }

    #[test]
    fn validate_c2b_confirmation_rejects_malformed_json() {
        let result = validate_c2b_confirmation(b"not json at all");
        assert!(result.is_err());
    }

    #[test]
    fn validate_c2b_confirmation_rejects_empty_json_object() {
        let result = validate_c2b_confirmation(b"{}");
        assert!(result.is_err(), "missing required fields should fail");
    }

    #[test]
    fn validate_c2b_confirmation_rejects_wrong_types() {
        let json = br#"{
            "TransactionType": 123,
            "TransID": "QEJ78H8JKR",
            "TransTime": "20240101120000",
            "TransAmount": "500.00",
            "BusinessShortCode": "174379",
            "BillRefNumber": "Test",
            "MSISDN": "254712345678"
        }"#;
        let result = validate_c2b_confirmation(json);
        assert!(result.is_err(), "TransactionType as number should fail");
    }

    #[test]
    fn validate_c2b_confirmation_rejects_invalid_domain_values() {
        let json = br#"{
            "TransactionType": "Pay Bill",
            "TransID": "QEJ78H8JKR",
            "TransTime": "not-a-time",
            "TransAmount": "0.00",
            "BusinessShortCode": "174379",
            "BillRefNumber": "Test",
            "MSISDN": "254712345678"
        }"#;
        assert!(matches!(
            validate_c2b_confirmation(json),
            Err(DarajaError::InvalidCallback(_))
        ));
    }

    #[test]
    fn c2b_deduplication_key_uses_transaction_id() {
        let callback = validate_c2b_confirmation(
            br#"{
            "TransactionType": "Pay Bill",
            "TransID": "QEJ78H8JKR",
            "TransTime": "20240101120000",
            "TransAmount": "500.00",
            "BusinessShortCode": "174379",
            "BillRefNumber": "Test",
            "MSISDN": "254712345678"
        }"#,
        )
        .unwrap();
        assert_eq!(
            c2b_callback_deduplication_key(&callback),
            "c2b:transaction:QEJ78H8JKR"
        );
    }
}
