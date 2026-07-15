use crate::error::DarajaError;
use crate::types::Environment;

pub fn validate_amount(amount: u64) -> Result<(), DarajaError> {
    if amount == 0 {
        return Err(DarajaError::Config(
            "amount must be at least one whole KES".into(),
        ));
    }
    Ok(())
}

pub fn validate_phone(phone: &str) -> Result<String, DarajaError> {
    let phone = phone.trim().trim_start_matches('+');
    let normalized = if let Some(local) = phone.strip_prefix('0') {
        format!("254{local}")
    } else {
        phone.to_string()
    };
    let valid_prefix = normalized
        .as_bytes()
        .get(3)
        .is_some_and(|prefix| matches!(prefix, b'1' | b'7'));
    if normalized.len() != 12
        || !normalized.starts_with("254")
        || !normalized.bytes().all(|byte| byte.is_ascii_digit())
        || !valid_prefix
    {
        return Err(DarajaError::Config(format!(
            "invalid Kenyan mobile number '{phone}': expected 07XXXXXXXX, 01XXXXXXXX, or 254XXXXXXXXX"
        )));
    }
    Ok(normalized)
}

pub fn validate_url(label: &str, value: &str, environment: Environment) -> Result<(), DarajaError> {
    let parsed = url::Url::parse(value)
        .map_err(|_| DarajaError::Config(format!("{label} must be an absolute URL")))?;
    if parsed.host_str().is_none() || !matches!(parsed.scheme(), "http" | "https") {
        return Err(DarajaError::Config(format!(
            "{label} must use http or https and include a host"
        )));
    }
    if matches!(environment, Environment::Production) && parsed.scheme() != "https" {
        return Err(DarajaError::Config(format!(
            "{label} must use HTTPS in production"
        )));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(DarajaError::Config(format!(
            "{label} must not contain embedded credentials"
        )));
    }
    Ok(())
}

pub fn validate_required_text(
    label: &str,
    value: &str,
    max_chars: usize,
) -> Result<(), DarajaError> {
    let length = value.chars().count();
    if value.trim().is_empty() || length > max_chars {
        return Err(DarajaError::Config(format!(
            "{label} must contain 1-{max_chars} characters"
        )));
    }
    Ok(())
}

pub fn validate_optional_text(
    label: &str,
    value: &str,
    max_chars: usize,
) -> Result<(), DarajaError> {
    if value.chars().count() > max_chars {
        return Err(DarajaError::Config(format!(
            "{label} must contain at most {max_chars} characters"
        )));
    }
    Ok(())
}

pub fn validate_shortcode(shortcode: &str) -> Result<(), DarajaError> {
    if !(5..=9).contains(&shortcode.len()) || !shortcode.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(DarajaError::Config(
            "shortcode must contain 5-9 digits".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Contract {
        phones: Vec<PhoneCase>,
        amounts: Vec<AmountCase>,
        urls: Vec<UrlCase>,
    }

    #[derive(Deserialize)]
    struct PhoneCase {
        input: String,
        normalized: Option<String>,
        valid: Option<bool>,
    }

    #[derive(Deserialize)]
    struct AmountCase {
        value: u64,
        valid: bool,
    }

    #[derive(Deserialize)]
    struct UrlCase {
        value: String,
        environment: String,
        valid: bool,
    }

    #[test]
    fn matches_cross_language_validation_contract() {
        let contract: Contract =
            serde_json::from_str(include_str!("../../fixtures/validation-contract.json")).unwrap();
        for case in contract.phones {
            let result = validate_phone(&case.input);
            assert_eq!(
                result.is_ok(),
                case.valid.unwrap_or(true),
                "phone {}",
                case.input
            );
            if let Some(expected) = case.normalized {
                assert_eq!(result.unwrap(), expected);
            }
        }
        for case in contract.amounts {
            assert_eq!(
                validate_amount(case.value).is_ok(),
                case.valid,
                "amount {}",
                case.value
            );
        }
        for case in contract.urls {
            let environment = if case.environment == "production" {
                Environment::Production
            } else {
                Environment::Sandbox
            };
            assert_eq!(
                validate_url("callback URL", &case.value, environment).is_ok(),
                case.valid,
                "URL {}",
                case.value
            );
        }
    }

    #[test]
    fn phone_validation_normalizes_supported_kenyan_formats() {
        assert_eq!(validate_phone("0712345678").unwrap(), "254712345678");
        assert_eq!(validate_phone("+254112345678").unwrap(), "254112345678");
    }

    #[test]
    fn phone_validation_rejects_malformed_values() {
        for value in ["", "712345678", "254812345678", "25471234abcd"] {
            assert!(validate_phone(value).is_err(), "accepted {value}");
        }
    }

    #[test]
    fn amount_must_be_positive() {
        assert!(validate_amount(0).is_err());
        assert!(validate_amount(1).is_ok());
    }

    #[test]
    fn production_urls_require_https() {
        assert!(
            validate_url(
                "callback URL",
                "http://example.com",
                Environment::Production
            )
            .is_err()
        );
        assert!(
            validate_url(
                "callback URL",
                "https://example.com",
                Environment::Production
            )
            .is_ok()
        );
        assert!(
            validate_url(
                "callback URL",
                "http://localhost:8181",
                Environment::Sandbox
            )
            .is_ok()
        );
    }
}
