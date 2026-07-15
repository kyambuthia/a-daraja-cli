// Security credential generation and STK Push password helpers.
//
// B2C / status / balance / reversal APIs require a SecurityCredential:
// RSA PKCS#1 v1.5 encrypt the initiator password with Safaricom's public
// certificate, then base64-encode the ciphertext.
//
// STK Push uses a different scheme:
//   base64(shortcode + passkey + timestamp (YYYYMMDDHHmmss))

use crate::certs::certificate_pem;
use crate::error::DarajaError;
use crate::types::Environment;
use base64::Engine;
use rand::rngs::OsRng;
use rsa::{Pkcs1v15Encrypt, RsaPublicKey, pkcs8::DecodePublicKey};
use std::convert::TryFrom;
use x509_cert::{
    certificate::Certificate,
    der::{DecodePem, referenced::OwnedToRef},
};

/// Generate a Daraja SecurityCredential from an initiator password and PEM certificate.
///
/// Parses the Safaricom API certificate (sandbox or production), RSA-encrypts
/// the initiator password with PKCS#1 v1.5, and base64-encodes the ciphertext.
/// Build a Daraja SecurityCredential for the initiator password in the given environment.
pub fn security_credential_for_environment(
    initiator_password: &str,
    environment: Environment,
) -> Result<String, DarajaError> {
    let pem = certificate_pem(environment)?;
    generate_security_credential(initiator_password, &pem)
}

pub fn generate_security_credential(
    initiator_password: &str,
    certificate_pem: &str,
) -> Result<String, DarajaError> {
    let public_key = rsa_public_key_from_pem(certificate_pem)?;
    let mut rng = OsRng;
    let encrypted = public_key
        .encrypt(&mut rng, Pkcs1v15Encrypt, initiator_password.as_bytes())
        .map_err(|e| DarajaError::Encryption(format!("RSA encryption failed: {e}")))?;

    Ok(base64::engine::general_purpose::STANDARD.encode(encrypted))
}

fn rsa_public_key_from_pem(pem: &str) -> Result<RsaPublicKey, DarajaError> {
    let pem = pem.trim();

    // Try certificate first
    if pem.contains("BEGIN CERTIFICATE") {
        let cert = Certificate::from_pem(pem.as_bytes()).map_err(|e| {
            DarajaError::Encryption(format!("failed to parse certificate PEM: {e}"))
        })?;
        let spki = cert.tbs_certificate.subject_public_key_info.owned_to_ref();
        return RsaPublicKey::try_from(spki).map_err(|e| {
            DarajaError::Encryption(format!(
                "certificate does not contain a valid RSA public key: {e}"
            ))
        });
    }

    // Try raw public key PEM
    RsaPublicKey::from_public_key_pem(pem)
        .map_err(|e| DarajaError::Encryption(format!("failed to parse public key PEM: {e}")))
}

/// Generate the encrypted password for STK Push requests.
///
/// Format: `base64(shortcode + passkey + timestamp)`
pub fn generate_password(shortcode: &str, passkey: &str, timestamp: &str) -> String {
    let concatenated = format!("{}{}{}", shortcode, passkey, timestamp);
    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, concatenated)
}

/// Format a `chrono::NaiveDateTime` as the Daraja timestamp format: `YYYYMMDDHHmmss`.
pub fn format_timestamp(dt: &chrono::NaiveDateTime) -> String {
    dt.format("%Y%m%d%H%M%S").to_string()
}

/// Return the current UTC time formatted as a Daraja timestamp.
pub fn current_timestamp() -> String {
    let now = chrono::Utc::now().naive_utc();
    format_timestamp(&now)
}

/// Normalize a Kenyan phone number to Daraja's preferred `254XXXXXXXXX` format.
///
/// Accepts inputs like `+2547...`, `07...`, or `2547...`.
pub fn normalize_phone(phone: &str) -> String {
    let phone = phone.trim().trim_start_matches('+');
    if let Some(local) = phone.strip_prefix('0') {
        format!("254{local}")
    } else {
        phone.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── generate_password ──────────────────────────────────

    #[test]
    fn password_is_base64_of_shortcode_passkey_timestamp() {
        // From API.md: sandbox shortcode=174379, passkey=bfb279f9..., timestamp=20240101120000
        let shortcode = "174379";
        let passkey = "bfb279f9aa9bdbcf158e97dd71a467cd2e0c893059b10f78e6b72ada1ed2c919";
        let timestamp = "20240101120000";

        let password = generate_password(shortcode, passkey, timestamp);

        // Verify it's valid base64
        let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &password)
            .expect("password should be valid base64");

        let expected_plain = format!("{}{}{}", shortcode, passkey, timestamp);
        assert_eq!(
            String::from_utf8(decoded).unwrap(),
            expected_plain,
            "decoded password should be concatenated shortcode + passkey + timestamp"
        );
    }

    #[test]
    fn password_changes_with_different_timestamps() {
        let shortcode = "174379";
        let passkey = "bfb279f9aa9bdbcf158e97dd71a467cd2e0c893059b10f78e6b72ada1ed2c919";

        let pw1 = generate_password(shortcode, passkey, "20240101120000");
        let pw2 = generate_password(shortcode, passkey, "20240101120001");

        assert_ne!(pw1, pw2, "passwords with different timestamps must differ");
    }

    #[test]
    fn password_changes_with_different_passkeys() {
        let shortcode = "174379";
        let timestamp = "20240101120000";

        let pw1 = generate_password(shortcode, "aaa", timestamp);
        let pw2 = generate_password(shortcode, "bbb", timestamp);

        assert_ne!(pw1, pw2, "passwords with different passkeys must differ");
    }

    #[test]
    fn password_handles_empty_inputs() {
        let pw = generate_password("", "", "");
        let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &pw)
            .expect("should be valid base64 even with empty inputs");
        assert!(decoded.is_empty());
    }

    // ── format_timestamp ────────────────────────────────────

    #[test]
    fn timestamp_format_is_yyyymmddhhmmss() {
        let dt = chrono::NaiveDate::from_ymd_opt(2024, 1, 1)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();
        let ts = format_timestamp(&dt);
        assert_eq!(ts, "20240101120000");
    }

    #[test]
    fn timestamp_pads_single_digit_months_and_days() {
        let dt = chrono::NaiveDate::from_ymd_opt(2024, 3, 5)
            .unwrap()
            .and_hms_opt(7, 3, 9)
            .unwrap();
        let ts = format_timestamp(&dt);
        assert_eq!(ts, "20240305070309");
    }

    #[test]
    fn timestamp_is_exactly_14_chars() {
        let dt = chrono::NaiveDate::from_ymd_opt(2026, 12, 31)
            .unwrap()
            .and_hms_opt(23, 59, 59)
            .unwrap();
        let ts = format_timestamp(&dt);
        assert_eq!(ts.len(), 14, "timestamp must be exactly 14 characters");
    }

    // ── current_timestamp ───────────────────────────────────

    #[test]
    fn current_timestamp_is_valid_format() {
        let ts = current_timestamp();
        assert_eq!(ts.len(), 14, "current_timestamp must be 14 chars");
        // Should be all digits
        assert!(ts.chars().all(|c| c.is_ascii_digit()), "must be all digits");
    }

    #[test]
    fn current_timestamp_is_recent() {
        let ts = current_timestamp();
        let year: i32 = ts[0..4].parse().unwrap();
        assert!((2024..=2099).contains(&year), "year should be reasonable");
    }

    // ── Edge cases ───────────────────────────────────────────

    #[test]
    fn format_timestamp_handles_leap_year() {
        let dt = chrono::NaiveDate::from_ymd_opt(2024, 2, 29)
            .unwrap()
            .and_hms_opt(23, 59, 59)
            .unwrap();
        assert_eq!(format_timestamp(&dt), "20240229235959");
    }

    #[test]
    fn format_timestamp_handles_year_2100() {
        let dt = chrono::NaiveDate::from_ymd_opt(2100, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        assert_eq!(format_timestamp(&dt), "21000101000000");
    }

    #[test]
    fn current_timestamp_monotonic() {
        let ts1 = current_timestamp();
        std::thread::sleep(std::time::Duration::from_millis(100));
        let ts2 = current_timestamp();
        // Either ts2 > ts1 or equal if we're unlucky with clock resolution
        assert!(
            ts2 >= ts1,
            "timestamps must never go backwards: {ts1} -> {ts2}"
        );
    }

    #[test]
    fn password_with_special_chars() {
        let pw = generate_password("!@#", "$%^", "&*()");
        let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &pw)
            .expect("special chars should produce valid base64");
        assert_eq!(String::from_utf8(decoded).unwrap(), "!@#$%^&*()");
    }

    #[test]
    fn password_with_unicode_inputs() {
        let pw = generate_password("💰", "💸", "20240101120000");
        let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &pw)
            .expect("unicode should encode cleanly");
        assert_eq!(String::from_utf8(decoded).unwrap(), "💰💸20240101120000");
    }

    #[test]
    fn password_with_very_long_input() {
        let long = "a".repeat(10_000);
        let pw = generate_password(&long, "b", "c");
        let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &pw)
            .expect("long input should encode");
        assert!(decoded.len() > 10_000);
    }

    #[test]
    fn generate_password_is_deterministic() {
        let pw1 = generate_password("a", "b", "c");
        let pw2 = generate_password("a", "b", "c");
        assert_eq!(pw1, pw2, "same inputs must produce same password");
    }

    // ── generate_security_credential ────────────────────────

    #[test]
    fn security_credential_from_rsa_public_key_pem() {
        use rand::rngs::OsRng;
        use rsa::{RsaPrivateKey, pkcs8::EncodePublicKey};

        let mut rng = OsRng;
        let private_key = RsaPrivateKey::new(&mut rng, 2048).expect("RSA key generation");
        let public_pem = private_key
            .to_public_key()
            .to_public_key_pem(rsa::pkcs8::LineEnding::LF)
            .expect("public key PEM encoding");
        let password = "Safaricom999!*!";

        let credential =
            generate_security_credential(password, &public_pem).expect("credential generation");

        let decoded =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &credential)
                .expect("security credential should be valid base64");

        assert!(!decoded.is_empty());
        assert_ne!(decoded, password.as_bytes());
    }

    #[test]
    fn security_credential_round_trips_with_generated_keypair() {
        use rand::rngs::OsRng;
        use rsa::{RsaPrivateKey, pkcs8::EncodePublicKey};

        let mut rng = OsRng;
        let private_key = RsaPrivateKey::new(&mut rng, 2048).expect("RSA key generation");
        let public_pem = private_key
            .to_public_key()
            .to_public_key_pem(rsa::pkcs8::LineEnding::LF)
            .expect("public key PEM encoding");
        let password = "test-initiator-password";

        let credential =
            generate_security_credential(password, &public_pem).expect("credential generation");

        let decoded =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &credential)
                .expect("security credential should be valid base64");

        let decrypted = private_key
            .decrypt(Pkcs1v15Encrypt, &decoded)
            .expect("RSA decryption");
        assert_eq!(String::from_utf8(decrypted).unwrap(), password);
    }

    #[test]
    fn security_credential_rejects_invalid_pem() {
        let result = generate_security_credential("password", "not a pem");
        assert!(result.is_err());
        match result {
            Err(DarajaError::Encryption(msg)) => {
                assert!(msg.contains("failed to parse"));
            }
            other => panic!("expected Encryption error, got {other:?}"),
        }
    }

    #[test]
    fn normalize_phone_accepts_plus_prefix() {
        assert_eq!(normalize_phone("+254743121299"), "254743121299");
    }

    #[test]
    fn normalize_phone_accepts_zero_prefix() {
        assert_eq!(normalize_phone("0743121299"), "254743121299");
    }

    #[test]
    fn normalize_phone_preserves_international_format() {
        assert_eq!(normalize_phone("254743121299"), "254743121299");
    }

    #[test]
    fn normalize_phone_handles_new_safaricom_prefixes() {
        assert_eq!(normalize_phone("0112345678"), "254112345678");
    }
}
