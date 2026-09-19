use crate::prelude::*;

pub(crate) fn sample_c2b_confirmation() -> C2bConfirmation {
    C2bConfirmation {
        transaction_type: "Pay Bill".into(),
        trans_id: "QEJ78H8JKR".into(),
        trans_time: "20240101120000".into(),
        trans_amount: "500.00".into(),
        business_short_code: "174379".into(),
        bill_ref_number: "Test".into(),
        msisdn: "254712345678".into(),
        first_name: Some("John".into()),
    }
}

pub(crate) async fn cmd_webhook(
    url: String,
    endpoint: Option<String>,
    json: bool,
) -> Result<(), DarajaError> {
    let endpoint = endpoint.as_deref().unwrap_or("confirm");
    if endpoint != "confirm" && endpoint != "validate" {
        return Err(DarajaError::Config(format!(
            "invalid endpoint '{endpoint}': expected 'confirm' or 'validate'"
        )));
    }

    let base = url.trim_end_matches('/');
    let target = if base.ends_with("/confirm") || base.ends_with("/validate") {
        base.to_string()
    } else {
        format!("{base}/{endpoint}")
    };
    let body = sample_c2b_confirmation();
    let payload = serde_json::to_string(&body)
        .map_err(|e| DarajaError::Config(format!("failed to serialize sample payload: {e}")))?;

    let resp = reqwest::Client::new()
        .post(&target)
        .header("Content-Type", "application/json")
        .body(payload)
        .send()
        .await
        .map_err(|e| DarajaError::Network(e.to_string()))?;

    let status = resp.status();
    let response_body = resp
        .text()
        .await
        .map_err(|e| DarajaError::Network(e.to_string()))?;

    if json {
        output::print_json(&serde_json::json!({
            "endpoint": target,
            "status": status.as_u16(),
            "body": response_body,
        }))?;
    } else {
        println!();
        println!("  Endpoint: {target}");
        println!("  Status:   {status}");
        println!("  Body:");
        println!("  {response_body}");
        println!();
    }

    Ok(())
}

pub(crate) async fn cmd_register(
    shortcode: Option<String>,
    response_type: String,
    confirm_url: String,
    validation_url: String,
    json: bool,
) -> Result<(), DarajaError> {
    let daraja = load_daraja().await?;
    let shortcode = match shortcode {
        Some(s) => s,
        None => load_shortcode()?,
    };

    let resp = register_urls(
        daraja.client(),
        &shortcode,
        &response_type,
        &confirm_url,
        &validation_url,
        &format!("register:{}", Uuid::new_v4()),
    )
    .await?;

    if json {
        output::print_json(&serde_json::json!({
            "conversation_id": resp.conversation_id,
            "originator_conversation_id": resp.originator_conversation_id,
            "response_description": resp.response_description,
        }))?;
    } else {
        println!("✓ C2B URLs registered");
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
    }

    Ok(())
}
