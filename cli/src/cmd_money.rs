use crate::prelude::*;

pub(crate) async fn cmd_simulate(
    phone: String,
    amount: String,
    reference: String,
    shortcode: Option<String>,
    json: bool,
) -> Result<(), DarajaError> {
    let sdk_key = load_sdk_key()?;
    if sdk_key.starts_with("dsk_live_") {
        return Err(DarajaError::Config(
            "C2B simulate is sandbox-only — switch to a dsk_test_ key".into(),
        ));
    }

    let amount = parse_kes_amount(&amount)?;
    let daraja = load_daraja().await?;
    let shortcode = match shortcode {
        Some(code) => code,
        None => load_shortcode()?,
    };
    let phone = validate_phone(&phone)?;

    let simulation_key = format!("sandbox:simulate:{}", Uuid::new_v4());
    let resp = simulate_c2b(
        daraja.client(),
        &shortcode,
        amount,
        &phone,
        &reference,
        &simulation_key,
    )
    .await?;

    if json {
        output::print_json(&serde_json::json!({
            "conversation_id": resp.conversation_id,
            "originator_conversation_id": resp.originator_conversation_id,
            "response_description": resp.response_description,
        }))?;
    } else {
        println!("✓ C2B payment simulated");
        println!();
        println!(
            "  Conversation ID:           {}",
            resp.conversation_id.as_deref().unwrap_or("(not provided)")
        );
        println!(
            "  Originator Conversation ID: {}",
            resp.originator_conversation_id
                .as_deref()
                .unwrap_or("(not provided)")
        );
        println!("  Response Description:      {}", resp.response_description);
        println!();
        println!("  Run `pesaraja receive` to catch the confirmation callback.");
        println!();
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)] // Mirrors the user-facing Clap command fields.
pub(crate) async fn cmd_send(
    to: String,
    amount: String,
    remarks: String,
    occasion: String,
    payment_type: SendType,
    result_url: Option<String>,
    timeout_url: Option<String>,
    yes: bool,
    dry_run: bool,
    idempotency_key: Option<String>,
    json: bool,
) -> Result<(), DarajaError> {
    let amount = parse_kes_amount(&amount)?;
    let phone = validate_phone(&to)?;
    validate_required_text("remarks", &remarks, 100)?;
    validate_optional_text("occasion", &occasion, 100)?;
    let (result_url, timeout_url) = resolve_async_urls(result_url, timeout_url)?;

    if dry_run {
        if configured_environment()? == Environment::Production && idempotency_key.is_none() {
            return Err(DarajaError::MissingIdempotencyKey);
        }
        print_dry_run(
            "B2C payment",
            &[
                ("Phone", phone.clone()),
                ("Amount", format!("KES {amount}")),
                ("Remarks", remarks.clone()),
                ("Occasion", occasion.clone()),
                (
                    "Type",
                    match payment_type {
                        SendType::Business => "business".into(),
                        SendType::Salary => "salary".into(),
                        SendType::Promotion => "promotion".into(),
                    },
                ),
                ("Result URL", result_url.clone()),
                ("Timeout URL", timeout_url.clone()),
                (
                    "Idempotency Key",
                    idempotency_key
                        .clone()
                        .unwrap_or_else(|| "generated and persisted in sandbox".into()),
                ),
            ],
        );
        return Ok(());
    }

    let daraja = load_daraja().await?;
    println!();
    confirm_payment(&phone, amount, &remarks, yes)?;
    println!();

    let payment_id = format!("pay_{}", Uuid::new_v4());
    let idempotency_key =
        operation_idempotency_key(daraja.client().environment(), idempotency_key, &payment_id)?;
    let mut payment_store = open_payment_store()?;
    if let Some(existing) = payment_store.get_by_idempotency_key(&idempotency_key)? {
        return Err(DarajaError::Config(format!(
            "idempotency key already belongs to local payment {}; inspect it before retrying",
            existing.id
        )));
    }
    payment_store.create(&NewPayment {
        id: payment_id.clone(),
        kind: PaymentKind::B2c,
        environment: match daraja.client().environment() {
            Environment::Sandbox => "sandbox".into(),
            Environment::Production => "production".into(),
        },
        shortcode: load_shortcode()?,
        amount_kes: amount,
        phone: Some(phone.clone()),
        reference: remarks.clone(),
        idempotency_key: Some(idempotency_key.clone()),
    })?;
    payment_store.transition(
        &payment_id,
        PaymentStatus::Submitted,
        Some("B2C request submitted"),
    )?;

    let command = payment_type.to_b2c_command();
    let resp = match b2c_payment_with_idempotency_key(
        daraja.client(),
        &phone,
        amount,
        command,
        &remarks,
        &occasion,
        &result_url,
        &timeout_url,
        &idempotency_key,
    )
    .await
    {
        Ok(response) => response,
        Err(error) => {
            return Err(persist_payment_error(
                &mut payment_store,
                &payment_id,
                error,
            ));
        }
    };
    payment_store
        .attach_conversation_ids(
            &payment_id,
            Some(&resp.conversation_id),
            Some(&resp.originator_conversation_id),
        )
        .map_err(|error| {
            DarajaError::LocalStateUncertain(format!(
                "record B2C provider identifiers for {payment_id}: {error}"
            ))
        })?;
    payment_store
        .transition(
            &payment_id,
            if resp.response_code == "0" {
                PaymentStatus::Pending
            } else {
                PaymentStatus::Failed
            },
            Some(&resp.response_description),
        )
        .map_err(|error| {
            DarajaError::LocalStateUncertain(format!(
                "record B2C provider outcome for {payment_id}: {error}"
            ))
        })?;

    if json {
        output::print_json(&serde_json::json!({
            "local_payment_id": payment_id,
            "idempotency_key": idempotency_key,
            "conversation_id": resp.conversation_id,
            "originator_conversation_id": resp.originator_conversation_id,
            "response_code": resp.response_code,
            "response_description": resp.response_description,
        }))?;
    } else {
        println!("✓ Payment sent");
        println!();
        println!("  Local Payment ID:          {payment_id}");
        println!("  Idempotency Key:           {idempotency_key}");
        println!("  Conversation ID:           {}", resp.conversation_id);
        println!(
            "  Originator Conversation ID: {}",
            resp.originator_conversation_id
        );
        println!("  Response Code:             {}", resp.response_code);
        println!("  Response Description:      {}", resp.response_description);
        println!();
    }

    Ok(())
}

pub(crate) async fn cmd_query(
    id: String,
    remarks: String,
    occasion: String,
    result_url: Option<String>,
    timeout_url: Option<String>,
    json: bool,
) -> Result<(), DarajaError> {
    let (result_url, timeout_url) = resolve_async_urls(result_url, timeout_url)?;
    let daraja = load_daraja().await?;

    let resp = query_status(
        daraja.client(),
        &id,
        &remarks,
        &occasion,
        &result_url,
        &timeout_url,
    )
    .await?;

    if json {
        output::print_json(&serde_json::json!({
            "conversation_id": resp.conversation_id,
            "originator_conversation_id": resp.originator_conversation_id,
            "response_code": resp.response_code,
            "response_description": resp.response_description,
        }))?;
    } else {
        println!("✓ Transaction query sent");
        println!();
        println!("  Conversation ID:           {}", resp.conversation_id);
        println!(
            "  Originator Conversation ID: {}",
            resp.originator_conversation_id
        );
        println!("  Response Code:             {}", resp.response_code);
        println!("  Response Description:      {}", resp.response_description);
        println!();
    }

    Ok(())
}

pub(crate) async fn cmd_balance(
    result_url: Option<String>,
    timeout_url: Option<String>,
    json: bool,
) -> Result<(), DarajaError> {
    let (result_url, timeout_url) = resolve_async_urls(result_url, timeout_url)?;
    let daraja = load_daraja().await?;

    let resp = query_balance(daraja.client(), &result_url, &timeout_url).await?;

    if json {
        output::print_json(&serde_json::json!({
            "conversation_id": resp.conversation_id,
            "originator_conversation_id": resp.originator_conversation_id,
            "response_code": resp.response_code,
            "response_description": resp.response_description,
            "note": "Balance arrives asynchronously at your result URL",
        }))?;
    } else {
        println!("✓ Balance query sent");
        println!();
        println!("  Conversation ID:           {}", resp.conversation_id);
        println!(
            "  Originator Conversation ID: {}",
            resp.originator_conversation_id
        );
        println!("  Response Code:             {}", resp.response_code);
        println!("  Response Description:      {}", resp.response_description);
        println!("  (Balance arrives asynchronously at your result URL)");
        println!();
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)] // Mirrors the user-facing Clap command fields.
pub(crate) async fn cmd_refund(
    id: String,
    amount: u64,
    receiver: String,
    receiver_type: String,
    remarks: String,
    occasion: String,
    result_url: Option<String>,
    timeout_url: Option<String>,
    dry_run: bool,
    yes: bool,
    idempotency_key: Option<String>,
    json: bool,
) -> Result<(), DarajaError> {
    validate_required_text("transaction ID", &id, 128)?;
    validate_merchant_shortcode(&receiver)?;
    validate_required_text("receiver identifier type", &receiver_type, 8)?;
    validate_required_text("remarks", &remarks, 100)?;
    validate_optional_text("occasion", &occasion, 100)?;
    let (result_url, timeout_url) = resolve_async_urls(result_url, timeout_url)?;
    if dry_run {
        if configured_environment()? == Environment::Production && idempotency_key.is_none() {
            return Err(DarajaError::MissingIdempotencyKey);
        }
        print_dry_run(
            "Reversal / refund",
            &[
                ("Transaction ID", id.clone()),
                ("Amount", format!("KES {amount}")),
                ("Receiver Party", receiver.clone()),
                ("Receiver Identifier Type", receiver_type.clone()),
                ("Remarks", remarks.clone()),
                ("Occasion", occasion.clone()),
                ("Result URL", result_url.clone()),
                ("Timeout URL", timeout_url.clone()),
                (
                    "Idempotency Key",
                    idempotency_key
                        .clone()
                        .unwrap_or_else(|| "generated and persisted in sandbox".into()),
                ),
            ],
        );
        return Ok(());
    }

    let daraja = load_daraja().await?;
    if !yes {
        let confirmed = Confirm::with_theme(&theme())
            .with_prompt(format!(
                "Reverse KES {amount} from transaction {id}? This can move money."
            ))
            .default(false)
            .interact()
            .map_err(|error| DarajaError::Config(format!("confirmation failed: {error}")))?;
        if !confirmed {
            return Err(DarajaError::Config("refund cancelled by user".into()));
        }
    }

    let payment_id = format!("pay_{}", Uuid::new_v4());
    let idempotency_key =
        operation_idempotency_key(daraja.client().environment(), idempotency_key, &payment_id)?;
    let mut payment_store = open_payment_store()?;
    if let Some(existing) = payment_store.get_by_idempotency_key(&idempotency_key)? {
        return Err(DarajaError::Config(format!(
            "idempotency key already belongs to local payment {}; inspect it before retrying",
            existing.id
        )));
    }
    payment_store.create(&NewPayment {
        id: payment_id.clone(),
        kind: PaymentKind::Reversal,
        environment: match daraja.client().environment() {
            Environment::Sandbox => "sandbox".into(),
            Environment::Production => "production".into(),
        },
        shortcode: load_shortcode()?,
        amount_kes: amount,
        phone: None,
        reference: id.clone(),
        idempotency_key: Some(idempotency_key.clone()),
    })?;
    payment_store.transition(
        &payment_id,
        PaymentStatus::Submitted,
        Some("reversal request submitted"),
    )?;

    let resp = match reverse_transaction_with_idempotency_key(
        daraja.client(),
        &id,
        amount,
        &receiver,
        &receiver_type,
        &remarks,
        &occasion,
        &result_url,
        &timeout_url,
        &idempotency_key,
    )
    .await
    {
        Ok(response) => response,
        Err(error) => {
            return Err(persist_payment_error(
                &mut payment_store,
                &payment_id,
                error,
            ));
        }
    };
    payment_store
        .attach_conversation_ids(
            &payment_id,
            Some(&resp.conversation_id),
            Some(&resp.originator_conversation_id),
        )
        .map_err(|error| {
            DarajaError::LocalStateUncertain(format!(
                "record reversal provider identifiers for {payment_id}: {error}"
            ))
        })?;
    payment_store
        .transition(
            &payment_id,
            if resp.response_code == "0" {
                PaymentStatus::Pending
            } else {
                PaymentStatus::Failed
            },
            Some(&resp.response_description),
        )
        .map_err(|error| {
            DarajaError::LocalStateUncertain(format!(
                "record reversal provider outcome for {payment_id}: {error}"
            ))
        })?;

    if json {
        output::print_json(&serde_json::json!({
            "local_payment_id": payment_id,
            "idempotency_key": idempotency_key,
            "conversation_id": resp.conversation_id,
            "originator_conversation_id": resp.originator_conversation_id,
            "response_code": resp.response_code,
            "response_description": resp.response_description,
        }))?;
    } else {
        println!("✓ Refund initiated");
        println!();
        println!("  Local Payment ID:           {payment_id}");
        println!("  Idempotency Key:            {idempotency_key}");
        println!("  Conversation ID:           {}", resp.conversation_id);
        println!(
            "  Originator Conversation ID: {}",
            resp.originator_conversation_id
        );
        println!("  Response Code:             {}", resp.response_code);
        println!("  Response Description:      {}", resp.response_description);
        println!();
    }

    Ok(())
}
