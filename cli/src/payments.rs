use crate::{PaymentCommands, load_daraja_with_base_url, open_payment_store, output};
use daraja_rs::api::stk_push::reconcile_stk_payment;
use daraja_rs::error::DarajaError;
use daraja_rs::payments::{Payment, PaymentKind, PaymentStatus};

pub async fn run(command: PaymentCommands, json: bool) -> Result<(), DarajaError> {
    match command {
        PaymentCommands::List { status } => {
            let status = status.as_deref().map(PaymentStatus::parse).transpose()?;
            let store = open_payment_store()?;
            let payments = store.list(status)?;
            if json {
                output::print_json(&payments.iter().map(payment_json).collect::<Vec<_>>())?;
            } else if payments.is_empty() {
                println!("No persisted payments.");
            } else {
                for payment in payments {
                    println!(
                        "{}  {:<9}  KES {:>8}  {}  {}",
                        payment.id,
                        payment.status.as_str(),
                        payment.amount_kes,
                        payment.reference,
                        payment.checkout_request_id.as_deref().unwrap_or("—")
                    );
                }
            }
        }
        PaymentCommands::Inspect { id } => {
            let store = open_payment_store()?;
            let payment = store
                .get(&id)?
                .ok_or_else(|| DarajaError::TransactionNotFound(id.clone()))?;
            let events = store.events(&id)?;
            if json {
                output::print_json(&serde_json::json!({
                    "payment": payment_json(&payment),
                    "events": events.iter().map(|event| serde_json::json!({
                        "from": event.from,
                        "to": event.to,
                        "detail": event.detail,
                        "created_at": event.created_at,
                    })).collect::<Vec<_>>(),
                }))?;
            } else {
                println!("Payment {}", payment.id);
                println!("  Kind:         {}", payment.kind.as_str());
                println!("  Status:       {}", payment.status.as_str());
                println!("  Environment:  {}", payment.environment);
                println!("  Amount:       KES {}", payment.amount_kes);
                println!(
                    "  Phone:        {}",
                    payment.phone.as_deref().unwrap_or("—")
                );
                println!("  Reference:    {}", payment.reference);
                println!(
                    "  Checkout ID:  {}",
                    payment.checkout_request_id.as_deref().unwrap_or("—")
                );
                println!("  Events:");
                for event in events {
                    println!(
                        "    {} -> {}{}",
                        event.from.as_deref().unwrap_or("—"),
                        event.to,
                        event
                            .detail
                            .as_deref()
                            .map(|detail| format!(" ({detail})"))
                            .unwrap_or_default()
                    );
                }
            }
        }
        PaymentCommands::Reconcile { id, base_url } => {
            let mut store = open_payment_store()?;
            let existing = store
                .get(&id)?
                .ok_or_else(|| DarajaError::TransactionNotFound(id.clone()))?;
            if existing.kind != PaymentKind::Stk {
                return Err(DarajaError::Config(format!(
                    "reconcile supports only persisted stk records; payment {} is {} and its asynchronous results cannot be authoritatively reconciled by this command",
                    existing.id,
                    existing.kind.as_str()
                )));
            }
            let daraja = load_daraja_with_base_url(base_url.as_deref()).await?;
            let payment = reconcile_stk_payment(daraja.client(), &mut store, &id).await?;
            if json {
                output::print_json(&payment_json(&payment))?;
            } else {
                println!(
                    "✓ Payment {} reconciled as {}",
                    payment.id,
                    payment.status.as_str()
                );
            }
        }
        PaymentCommands::Retry { id, dry_run } => {
            let store = open_payment_store()?;
            let payment = store
                .get(&id)?
                .ok_or_else(|| DarajaError::TransactionNotFound(id.clone()))?;
            if payment.status != PaymentStatus::Failed {
                return Err(DarajaError::Config(format!(
                    "payment is {}; reconcile unknown/pending payments before considering a retry",
                    payment.status.as_str()
                )));
            }
            if !dry_run {
                return Err(DarajaError::Config(
                    "retry is currently plan-only; pass --dry-run to inspect the safe retry payload"
                        .into(),
                ));
            }
            let retry_key = format!(
                "{}:retry",
                payment.idempotency_key.as_deref().unwrap_or(&payment.id)
            );
            let value = serde_json::json!({
                "original_payment_id": payment.id,
                "status": payment.status.as_str(),
                "phone": payment.phone,
                "amount_kes": payment.amount_kes,
                "reference": payment.reference,
                "new_idempotency_key": retry_key,
                "request_sent": false,
            });
            if json {
                output::print_json(&value)?;
            } else {
                println!("Retry plan (no request sent):");
                println!("  Original payment: {}", payment.id);
                println!("  Amount:           KES {}", payment.amount_kes);
                println!("  New idempotency:  {retry_key}");
            }
        }
    }
    Ok(())
}

fn payment_json(payment: &Payment) -> serde_json::Value {
    serde_json::json!({
        "id": payment.id,
        "kind": payment.kind.as_str(),
        "environment": payment.environment,
        "shortcode": payment.shortcode,
        "amount_kes": payment.amount_kes,
        "phone": payment.phone,
        "reference": payment.reference,
        "idempotency_key": payment.idempotency_key,
        "merchant_request_id": payment.merchant_request_id,
        "checkout_request_id": payment.checkout_request_id,
        "conversation_id": payment.conversation_id,
        "originator_conversation_id": payment.originator_conversation_id,
        "transaction_id": payment.transaction_id,
        "receipt_number": payment.receipt_number,
        "status": payment.status.as_str(),
        "created_at": payment.created_at,
        "updated_at": payment.updated_at,
    })
}
