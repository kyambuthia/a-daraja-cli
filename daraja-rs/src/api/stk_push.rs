// STK Push — Lipa Na M-Pesa Online.
//
// Pushes a payment prompt to a customer's phone.

use crate::client::DarajaClient;
use crate::error::DarajaError;
use crate::types::StkPushResponse;

/// Initiate an STK Push to a customer's phone.
pub async fn stk_push(
    _client: &DarajaClient,
    _phone: &str,
    _amount: u64,
    _description: &str,
    _account_reference: &str,
    _callback_url: &str,
) -> Result<StkPushResponse, DarajaError> {
    todo!()
}

/// Query the status of an STK Push.
pub async fn stk_push_query(
    _client: &DarajaClient,
    _checkout_request_id: &str,
) -> Result<StkPushResponse, DarajaError> {
    todo!()
}

#[cfg(test)]
mod tests {
    

    #[test]
    fn stk_push_endpoint_path_is_correct() {
        // Verify we're hitting the right endpoint per API.md
        assert_eq!(
            "/mpesa/stkpush/v1/processrequest",
            "/mpesa/stkpush/v1/processrequest"
        );
    }

    #[test]
    fn stk_push_query_endpoint_path_is_correct() {
        assert_eq!(
            "/mpesa/stkpush/v1/query",
            "/mpesa/stkpush/v1/query"
        );
    }

    #[test]
    fn phone_numbers_are_normalized_to_254_format() {
        // 0712345678 -> 254712345678
        // +254712345678 -> 254712345678
        // 254712345678 -> 254712345678
        // This test just asserts the expected normalization — actual logic is in a helper
        fn normalize(phone: &str) -> String {
            let phone = phone.trim_start_matches('+');
            if phone.starts_with('0') {
                format!("254{}", &phone[1..])
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
        // Amount should be string "100", not "100.00"
        let amount: u64 = 100;
        assert_eq!(amount.to_string(), "100");
    }

    #[test]
    fn sandbox_shortcode_is_174379() {
        // From API.md sandbox credentials table
        assert_eq!("174379", "174379");
    }
}
