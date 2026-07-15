use daraja_rs::error::DarajaError;
use daraja_rs::validation::validate_amount;

pub fn parse_kes_amount(amount: &str) -> Result<u64, DarajaError> {
    let amount = amount.trim();
    if amount.is_empty() {
        return Err(DarajaError::Config("amount cannot be empty".into()));
    }

    let parsed = if let Some((whole, fraction)) = amount.split_once('.') {
        if whole.is_empty()
            || fraction.is_empty()
            || !whole.chars().all(|character| character.is_ascii_digit())
            || !fraction.chars().all(|character| character.is_ascii_digit())
        {
            return Err(DarajaError::Config(format!(
                "invalid amount '{amount}': expected a whole number of KES"
            )));
        }
        if fraction.chars().any(|character| character != '0') {
            return Err(DarajaError::Config(format!(
                "invalid amount '{amount}': fractional KES amounts are not supported"
            )));
        }
        whole.parse()
    } else {
        if !amount.chars().all(|character| character.is_ascii_digit()) {
            return Err(DarajaError::Config(format!(
                "invalid amount '{amount}': expected a whole number of KES"
            )));
        }
        amount.parse()
    }
    .map_err(|_| DarajaError::Config(format!("invalid amount '{amount}'")))?;

    validate_amount(parsed)?;
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_positive_whole_kes() {
        assert_eq!(parse_kes_amount("100").unwrap(), 100);
        assert_eq!(parse_kes_amount("100.00").unwrap(), 100);
    }

    #[test]
    fn rejects_zero_fractional_and_malformed_values() {
        for value in ["", "0", "100.50", "-1", "KES 100", "1."] {
            assert!(parse_kes_amount(value).is_err(), "accepted {value}");
        }
    }
}
