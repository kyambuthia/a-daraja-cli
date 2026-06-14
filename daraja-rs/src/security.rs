// Password generation and timestamp formatting for STK Push.
//
// Daraja requires a base64-encoded password built from:
//   shortcode + passkey + timestamp (YYYYMMDDHHmmss)

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
        let decoded = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &password,
        )
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
        let decoded = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &pw,
        )
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
        assert!(year >= 2024 && year <= 2099, "year should be reasonable");
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
        assert!(ts2 >= ts1, "timestamps must never go backwards: {ts1} -> {ts2}");
    }

    #[test]
    fn password_with_special_chars() {
        let pw = generate_password("!@#", "$%^", "&*()");
        let decoded = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &pw,
        )
        .expect("special chars should produce valid base64");
        assert_eq!(String::from_utf8(decoded).unwrap(), "!@#$%^&*()");
    }

    #[test]
    fn password_with_unicode_inputs() {
        let pw = generate_password("💰", "💸", "20240101120000");
        let decoded = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &pw,
        )
        .expect("unicode should encode cleanly");
        assert_eq!(String::from_utf8(decoded).unwrap(), "💰💸20240101120000");
    }

    #[test]
    fn password_with_very_long_input() {
        let long = "a".repeat(10_000);
        let pw = generate_password(&long, "b", "c");
        let decoded = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &pw,
        )
        .expect("long input should encode");
        assert!(decoded.len() > 10_000);
    }

    #[test]
    fn generate_password_is_deterministic() {
        let pw1 = generate_password("a", "b", "c");
        let pw2 = generate_password("a", "b", "c");
        assert_eq!(pw1, pw2, "same inputs must produce same password");
    }
}
