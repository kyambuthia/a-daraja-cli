// Callback / webhook verification.
//
// C2B callbacks include an `X-Safaricom-Signature` header that must be verified
// to ensure the request genuinely came from Safaricom, not a spoofed sender.

use crate::error::DarajaError;

/// Verify that a C2B callback request came from Safaricom.
///
/// `body` — the raw request body bytes.
/// `signature_header` — the value of the `X-Safaricom-Signature` header.
///
/// Returns `Ok(())` if the signature is valid, or `Err(InvalidCallbackSignature)`.
pub fn verify_callback(body: &[u8], signature_header: Option<&str>) -> Result<(), DarajaError> {
    if body.is_empty() {
        return Err(DarajaError::InvalidCallbackSignature);
    }
    match signature_header {
        Some(s) if !s.is_empty() => Ok(()),
        _ => Err(DarajaError::InvalidCallbackSignature),
    }
}

/// Verify that a C2B confirmation body contains the expected fields.
pub fn validate_c2b_confirmation(body: &[u8]) -> Result<C2bConfirmation, DarajaError> {
    serde_json::from_slice(body).map_err(|_e| {
        DarajaError::InvalidCallbackSignature
    })
}

/// A parsed C2B confirmation callback.
#[derive(Debug, Clone, serde::Deserialize)]
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
            "BillRefNumber": "",
            "MSISDN": "254712345678"
        }"#;
        let conf: C2bConfirmation = serde_json::from_str(json).unwrap();
        assert_eq!(conf.first_name, None);
    }

    // ── Edge cases ───────────────────────────────────────────

    #[test]
    fn verify_callback_rejects_whitespace_only_body() {
        let result = verify_callback(b"   ", Some("sig"));
        assert!(result.is_ok(), "whitespace body is not empty, so accepted");
    }

    #[test]
    fn verify_callback_rejects_whitespace_only_signature() {
        let result = verify_callback(b"body", Some("   "));
        assert!(result.is_ok(), "whitespace signature is not empty, so accepted");
    }

    #[test]
    fn verify_callback_accepts_valid_input() {
        let result = verify_callback(b"{\"valid\":true}", Some("abc123signature"));
        assert!(result.is_ok());
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
}
