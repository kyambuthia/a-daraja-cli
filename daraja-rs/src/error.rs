// Human-readable error types.
//
// Every error maps to a sentence. Never a raw status code.
// See docs/ARCHITECTURE.md for the full error design.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DarajaError {
    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("The phone number '{0}' is not registered on M-Pesa.")]
    PhoneNotRegistered(String),

    #[error("STK push cancelled: user declined the payment.")]
    StkCancelled,

    #[error("STK push timed out: user did not enter PIN within 60 seconds.")]
    StkTimeout,

    #[error("Insufficient funds in your M-Pesa account (balance: KES {0}).")]
    InsufficientFunds(i64),

    #[error("Rate limited: too many requests. Wait {0} seconds.")]
    RateLimited(u64),

    #[error("Daraja API returned an unknown error: '{0}'. Please report this.")]
    UnknownApiError(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Daraja API request failed (HTTP {status}, code {code}): {message}")]
    Api {
        status: u16,
        code: String,
        message: String,
    },

    #[error("Invalid callback signature: request did not come from Safaricom.")]
    InvalidCallbackSignature,

    #[error("Invalid callback payload: {0}")]
    InvalidCallback(String),

    #[error("Callback signature verification is not configured; refusing to trust this callback.")]
    CallbackVerificationUnavailable,

    #[error("Idempotency conflict: a payment with this key is already in flight.")]
    IdempotencyConflict,

    #[error("Idempotency key was already used with different request parameters.")]
    IdempotencyKeyReused,

    #[error(
        "A previous request with this idempotency key failed; use a new key only after reviewing the failure."
    )]
    IdempotencyPreviousFailed,

    #[error("Payment outcome is unknown; reconcile the transaction before retrying.")]
    IdempotencyOutcomeUnknown,

    #[error("Invalid payment state transition: {from} -> {to}")]
    InvalidPaymentTransition { from: String, to: String },

    #[error("Transaction not found: {0}")]
    TransactionNotFound(String),

    #[error("Reversal failed: {0}")]
    ReversalFailed(String),

    #[error("DARAJA_KEY environment variable is not set.")]
    MissingApiKey,

    #[error("SDK key has wrong environment prefix. Expected '{expected}', got '{actual}'.")]
    WrongEnvironmentPrefix { expected: String, actual: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_are_human_readable() {
        let err = DarajaError::PhoneNotRegistered("0712345678".into());
        let msg = err.to_string();
        assert!(msg.contains("0712345678"));
        assert!(msg.contains("not registered"));
    }

    #[test]
    fn auth_error_includes_reason() {
        let err = DarajaError::AuthFailed("invalid consumer key".into());
        assert!(err.to_string().contains("invalid consumer key"));
    }

    #[test]
    fn rate_limit_error_includes_seconds() {
        let err = DarajaError::RateLimited(5);
        assert!(err.to_string().contains("5 seconds"));
    }

    #[test]
    fn insufficient_funds_includes_balance() {
        let err = DarajaError::InsufficientFunds(12);
        assert!(err.to_string().contains("KES 12"));
    }

    #[test]
    fn unknown_api_error_includes_code() {
        let err = DarajaError::UnknownApiError("999".into());
        assert!(err.to_string().contains("999"));
        assert!(err.to_string().contains("report this"));
    }

    #[test]
    fn all_errors_can_be_constructed() {
        // Compile-time check that all variants are constructible.
        let errors = vec![
            DarajaError::AuthFailed("test".into()),
            DarajaError::PhoneNotRegistered("test".into()),
            DarajaError::StkCancelled,
            DarajaError::StkTimeout,
            DarajaError::InsufficientFunds(0),
            DarajaError::RateLimited(0),
            DarajaError::UnknownApiError("test".into()),
            DarajaError::Config("test".into()),
            DarajaError::Encryption("test".into()),
            DarajaError::Network("test".into()),
            DarajaError::Api {
                status: 400,
                code: "invalid_request".into(),
                message: "request rejected".into(),
            },
            DarajaError::InvalidCallbackSignature,
            DarajaError::InvalidCallback("test".into()),
            DarajaError::CallbackVerificationUnavailable,
            DarajaError::IdempotencyConflict,
            DarajaError::IdempotencyKeyReused,
            DarajaError::IdempotencyPreviousFailed,
            DarajaError::IdempotencyOutcomeUnknown,
            DarajaError::InvalidPaymentTransition {
                from: "created".into(),
                to: "reversed".into(),
            },
            DarajaError::TransactionNotFound("test".into()),
            DarajaError::ReversalFailed("test".into()),
            DarajaError::MissingApiKey,
            DarajaError::WrongEnvironmentPrefix {
                expected: "test".into(),
                actual: "live".into(),
            },
        ];
        assert_eq!(errors.len(), 23);
    }

    #[test]
    fn daraja_error_implements_std_error() {
        fn assert_error<T: std::error::Error>() {}
        assert_error::<DarajaError>();
    }
}
