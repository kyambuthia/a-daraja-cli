//! Pure argument-to-request mapping for money-moving commands.
//!
//! Each mapper validates raw CLI strings exactly as the command handlers did
//! and returns an SDK-ready value object. Mappers perform no I/O: they never
//! read credentials, touch the environment, or call Daraja. URL resolution,
//! idempotency-key derivation, confirmation prompts, persistence, and output
//! stay in the `cmd_*` handlers so payment behavior is unchanged.

use crate::prelude::*;

/// Validated B2C (`send`) arguments ready for the SDK call and ledger.
#[derive(Debug)]
pub(crate) struct B2cPaymentArgs {
    pub(crate) phone: String,
    pub(crate) amount_kes: u64,
    pub(crate) command: B2cCommand,
    pub(crate) remarks: String,
    pub(crate) occasion: String,
}

/// Validate raw `send` arguments in handler order and map them to SDK types.
pub(crate) fn map_b2c_args(
    to: &str,
    amount: &str,
    remarks: &str,
    occasion: &str,
    payment_type: SendType,
) -> Result<B2cPaymentArgs, DarajaError> {
    let amount_kes = parse_kes_amount(amount)?;
    let phone = validate_phone(to)?;
    validate_required_text("remarks", remarks, 100)?;
    validate_optional_text("occasion", occasion, 100)?;
    Ok(B2cPaymentArgs {
        phone,
        amount_kes,
        command: payment_type.to_b2c_command(),
        remarks: remarks.to_string(),
        occasion: occasion.to_string(),
    })
}

/// Validated reversal (`refund`) arguments ready for the SDK call and ledger.
///
/// `amount_kes` arrives already parsed by the caller (`parse_kes_amount` in
/// `main.rs`), so only the text fields are validated here, in handler order.
#[derive(Debug)]
pub(crate) struct ReversalArgs {
    pub(crate) transaction_id: String,
    pub(crate) amount_kes: u64,
    pub(crate) receiver: String,
    pub(crate) receiver_type: String,
    pub(crate) remarks: String,
    pub(crate) occasion: String,
}

/// Validate raw `refund` arguments in handler order.
pub(crate) fn map_reversal_args(
    id: &str,
    amount_kes: u64,
    receiver: &str,
    receiver_type: &str,
    remarks: &str,
    occasion: &str,
) -> Result<ReversalArgs, DarajaError> {
    validate_required_text("transaction ID", id, 128)?;
    validate_merchant_shortcode(receiver)?;
    validate_required_text("receiver identifier type", receiver_type, 8)?;
    validate_required_text("remarks", remarks, 100)?;
    validate_optional_text("occasion", occasion, 100)?;
    Ok(ReversalArgs {
        transaction_id: id.to_string(),
        amount_kes,
        receiver: receiver.to_string(),
        receiver_type: receiver_type.to_string(),
        remarks: remarks.to_string(),
        occasion: occasion.to_string(),
    })
}

/// Validated STK push (`prompt`) arguments.
///
/// Mirrors `cmd_collect::cmd_prompt` validation (amount, phone, description,
/// account reference, then transaction-type mapping). Callback/base-URL
/// resolution stays with the caller because it reads the environment.
#[derive(Debug)]
pub(crate) struct StkPushArgs {
    pub(crate) phone: String,
    pub(crate) amount_kes: u64,
    pub(crate) description: String,
    pub(crate) reference: String,
    pub(crate) transaction_type: StkTransactionType,
}

/// Validate raw `prompt` arguments in handler order.
pub(crate) fn map_stk_push_args(
    phone: &str,
    amount: &str,
    description: &str,
    reference: &str,
    payment_type: PromptType,
) -> Result<StkPushArgs, DarajaError> {
    let amount_kes = parse_kes_amount(amount)?;
    let phone = validate_phone(phone)?;
    validate_required_text("description", description, 100)?;
    validate_required_text("account reference", reference, 100)?;
    Ok(StkPushArgs {
        phone,
        amount_kes,
        description: description.to_string(),
        reference: reference.to_string(),
        transaction_type: payment_type.to_stk_transaction_type(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn b2c_maps_valid_args_and_normalizes_phone() {
        let args = map_b2c_args("0712345678", "100", "Salary", "June", SendType::Business)
            .expect("valid B2C args");
        assert_eq!(args.phone, "254712345678");
        assert_eq!(args.amount_kes, 100);
        assert_eq!(args.command, B2cCommand::BusinessPayment);
        assert_eq!(args.remarks, "Salary");
        assert_eq!(args.occasion, "June");
    }

    #[test]
    fn b2c_maps_each_payment_type_to_its_command() {
        let business = map_b2c_args("254712345678", "10", "R", "", SendType::Business).unwrap();
        let salary = map_b2c_args("254712345678", "10", "R", "", SendType::Salary).unwrap();
        let promotion = map_b2c_args("254712345678", "10", "R", "", SendType::Promotion).unwrap();
        assert_eq!(business.command, B2cCommand::BusinessPayment);
        assert_eq!(salary.command, B2cCommand::SalaryPayment);
        assert_eq!(promotion.command, B2cCommand::PromotionPayment);
    }

    #[test]
    fn b2c_rejects_invalid_amount_phone_remarks_and_occasion() {
        assert!(map_b2c_args("254712345678", "0", "R", "", SendType::Business).is_err());
        assert!(map_b2c_args("not-a-phone", "10", "R", "", SendType::Business).is_err());
        assert!(map_b2c_args("254712345678", "10", "  ", "", SendType::Business).is_err());
        assert!(
            map_b2c_args(
                "254712345678",
                "10",
                "R",
                &"o".repeat(101),
                SendType::Business
            )
            .is_err()
        );
    }

    #[test]
    fn b2c_reports_amount_errors_before_phone_errors() {
        let err = map_b2c_args("not-a-phone", "0", "R", "", SendType::Business)
            .expect_err("amount must fail first");
        assert!(err.to_string().contains("amount"));
    }

    #[test]
    fn reversal_maps_valid_args() {
        let args = map_reversal_args("QEJ78H8JKR", 100, "600000", "11", "Wrong send", "")
            .expect("valid reversal args");
        assert_eq!(args.transaction_id, "QEJ78H8JKR");
        assert_eq!(args.amount_kes, 100);
        assert_eq!(args.receiver, "600000");
        assert_eq!(args.receiver_type, "11");
        assert_eq!(args.remarks, "Wrong send");
    }

    #[test]
    fn reversal_rejects_invalid_fields() {
        assert!(map_reversal_args("", 100, "600000", "11", "R", "").is_err());
        assert!(map_reversal_args("QEJ78H8JKR", 100, "07MSDN", "11", "R", "").is_err());
        assert!(map_reversal_args("QEJ78H8JKR", 100, "600000", "", "R", "").is_err());
        assert!(map_reversal_args("QEJ78H8JKR", 100, "600000", "11", "", "").is_err());
        assert!(
            map_reversal_args("QEJ78H8JKR", 100, "600000", "11", "R", &"o".repeat(101)).is_err()
        );
    }

    #[test]
    fn stk_maps_valid_args_and_transaction_types() {
        let paybill =
            map_stk_push_args("0712345678", "100", "Payment", "INV-1", PromptType::Paybill)
                .expect("valid STK args");
        assert_eq!(paybill.phone, "254712345678");
        assert_eq!(paybill.amount_kes, 100);
        assert_eq!(paybill.transaction_type, StkTransactionType::PayBill);

        let till = map_stk_push_args("254712345678", "50", "Payment", "INV-2", PromptType::Till)
            .expect("valid STK args");
        assert_eq!(till.transaction_type, StkTransactionType::Till);
    }

    #[test]
    fn stk_rejects_invalid_fields() {
        assert!(map_stk_push_args("254712345678", "0", "D", "R", PromptType::Paybill).is_err());
        assert!(map_stk_push_args("bad", "10", "D", "R", PromptType::Paybill).is_err());
        assert!(map_stk_push_args("254712345678", "10", "", "R", PromptType::Paybill).is_err());
        assert!(map_stk_push_args("254712345678", "10", "D", "", PromptType::Paybill).is_err());
    }

    #[test]
    fn stk_reports_amount_errors_before_phone_errors() {
        let err = map_stk_push_args("not-a-phone", "0", "D", "R", PromptType::Paybill)
            .expect_err("amount must fail first");
        assert!(err.to_string().contains("amount"));
    }

    #[test]
    fn reversal_reports_transaction_id_errors_before_receiver_errors() {
        let err = map_reversal_args("", 100, "07MSDN", "11", "R", "")
            .expect_err("transaction ID must fail first");
        assert!(err.to_string().contains("transaction ID"));
    }
}
