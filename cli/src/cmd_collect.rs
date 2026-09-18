use crate::prelude::*;

#[allow(clippy::too_many_arguments)] // Mirrors the user-facing Clap command fields.
pub(crate) async fn cmd_prompt(
    phone: String,
    amount: String,
    description: String,
    reference: String,
    callback_url: Option<String>,
    payment_type: PromptType,
    yes: bool,
    dry_run: bool,
    idempotency_key: Option<String>,
    base_url: Option<String>,
    json: bool,
) -> Result<(), DarajaError> {
    let amount = parse_kes_amount(&amount)?;
    let phone = validate_phone(&phone)?;
    validate_required_text("description", &description, 100)?;
    validate_required_text("account reference", &reference, 100)?;

    let callback = resolve_callback_url(callback_url)?;
    let base_url = base_url
        .as_deref()
        .map(validate_loopback_base_url)
        .transpose()?;

    if dry_run {
        if configured_environment()? == Environment::Production && idempotency_key.is_none() {
            return Err(DarajaError::MissingIdempotencyKey);
        }
        let fields = [
            ("Phone", phone.clone()),
            ("Amount", format!("KES {amount}")),
            ("Description", description.clone()),
            ("Reference", reference.clone()),
            ("Callback URL", callback.clone()),
            (
                "Idempotency Key",
                idempotency_key
                    .clone()
                    .unwrap_or_else(|| "generated and persisted in sandbox".into()),
            ),
            (
                "Type",
                match payment_type {
                    PromptType::Paybill => "paybill".into(),
                    PromptType::Till => "till".into(),
                },
            ),
        ];
        if json {
            output::print_json(&serde_json::json!({
                "operation": "stk_push",
                "phone": phone,
                "amount_kes": amount,
                "description": description,
                "reference": reference,
                "callback_url": callback,
                "idempotency_key": idempotency_key,
                "request_sent": false,
            }))?;
        } else {
            print_dry_run("STK Push", &fields);
        }
        return Ok(());
    }

    let daraja = load_daraja_with_base_url(base_url.as_deref()).await?;
    println!();
    confirm_payment(&phone, amount, &description, yes)?;
    println!();

    let payment_id = format!("pay_{}", Uuid::new_v4());
    let idempotency_key =
        operation_idempotency_key(daraja.client().environment(), idempotency_key, &payment_id)?;
    let mut payment_store = open_payment_store()?;
    if let Some(existing) = payment_store.get_by_idempotency_key(&idempotency_key)? {
        return Err(DarajaError::Config(format!(
            "idempotency key already belongs to local payment {}; inspect or reconcile it",
            existing.id
        )));
    }
    payment_store.create(&NewPayment {
        id: payment_id.clone(),
        kind: PaymentKind::Stk,
        environment: match daraja.client().environment() {
            Environment::Sandbox => "sandbox".into(),
            Environment::Production => "production".into(),
        },
        shortcode: load_shortcode()?,
        amount_kes: amount,
        phone: Some(phone.clone()),
        reference: reference.clone(),
        idempotency_key: Some(idempotency_key.clone()),
    })?;
    payment_store.transition(
        &payment_id,
        PaymentStatus::Submitted,
        Some("STK request submitted"),
    )?;

    let transaction_type = payment_type.to_stk_transaction_type();
    let resp = stk_push_with_type_and_idempotency_key(
        daraja.client(),
        &phone,
        amount,
        &description,
        &reference,
        &callback,
        transaction_type,
        &idempotency_key,
    )
    .await;
    let resp = match resp {
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
        .attach_provider_ids(
            &payment_id,
            Some(&resp.merchant_request_id),
            Some(&resp.checkout_request_id),
            None,
            None,
        )
        .map_err(|error| {
            DarajaError::LocalStateUncertain(format!(
                "record STK provider identifiers for {payment_id}: {error}"
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
                "record STK provider outcome for {payment_id}: {error}"
            ))
        })?;

    if json {
        output::print_json(&serde_json::json!({
            "local_payment_id": payment_id,
            "idempotency_key": idempotency_key,
            "merchant_request_id": resp.merchant_request_id,
            "checkout_request_id": resp.checkout_request_id,
            "response_code": resp.response_code,
            "response_description": resp.response_description,
            "customer_message": resp.customer_message,
        }))?;
    } else {
        println!("✓ Payment prompt sent");
        println!();
        println!("  Local Payment ID:      {payment_id}");
        println!("  Idempotency Key:       {idempotency_key}");
        println!("  Merchant Request ID:  {}", resp.merchant_request_id);
        println!("  Checkout Request ID:  {}", resp.checkout_request_id);
        println!("  Response Code:        {}", resp.response_code);
        println!("  Response Description: {}", resp.response_description);
        println!("  Customer Message:     {}", resp.customer_message);
        println!();
    }

    Ok(())
}

pub(crate) async fn cmd_prompt_status(
    checkout_id: String,
    base_url: Option<String>,
    json: bool,
) -> Result<(), DarajaError> {
    let daraja = load_daraja_with_base_url(base_url.as_deref()).await?;

    let resp = stk_push_query(daraja.client(), &checkout_id).await?;

    if json {
        output::print_json(&serde_json::json!({
            "merchant_request_id": resp.merchant_request_id,
            "checkout_request_id": resp.checkout_request_id,
            "response_code": resp.response_code,
            "response_description": resp.response_description,
            "customer_message": resp.customer_message,
        }))?;
    } else {
        println!("✓ Prompt status retrieved");
        println!();
        println!("  Merchant Request ID:  {}", resp.merchant_request_id);
        println!("  Checkout Request ID:  {}", resp.checkout_request_id);
        println!("  Response Code:        {}", resp.response_code);
        println!("  Response Description: {}", resp.response_description);
        println!(
            "  Customer Message:     {}",
            resp.customer_message.as_deref().unwrap_or("(not provided)")
        );
        println!();
    }

    Ok(())
}
