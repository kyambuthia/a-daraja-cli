use crate::auth::get_token;
use crate::error::DarajaError;
use crate::types::{Credentials, Environment, StkPushRequest, StkPushResponse};
use chrono::Utc;
use reqwest::Client;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct StkPushPayload {
    #[serde(rename = "BusinessShortCode")]
    business_short_code: String,
    #[serde(rename = "Password")]
    password: String,
    #[serde(rename = "Timestamp")]
    timestamp: String,
    #[serde(rename = "TransactionType")]
    transaction_type: String,
    #[serde(rename = "Amount")]
    amount: String,
    #[serde(rename = "PartyA")]
    party_a: String,
    #[serde(rename = "PartyB")]
    party_b: String,
    #[serde(rename = "PhoneNumber")]
    phone_number: String,
    #[serde(rename = "CallBackURL")]
    callback_url: String,
    #[serde(rename = "AccountReference")]
    account_reference: String,
    #[serde(rename = "TransactionDesc")]
    transaction_desc: String,
}

pub async fn stk_push(
    client: &Client,
    credentials: &Credentials,
    request: &StkPushRequest,
) -> Result<StkPushResponse, DarajaError> {
    let token = get_token(client, credentials).await?;
    let timestamp = Utc::now().format("%Y%m%d%H%M%S").to_string();
    let password = credentials.password(&timestamp);

    let phone = normalize_phone(&request.phone)?;

    let callback_url = match &request.callback_url {
        Some(url) => url.clone(),
        None => match credentials.environment {
            Environment::Sandbox => "https://sandbox.safaricom.co.ke/mpesa/result".into(),
            Environment::Production => {
                return Err(DarajaError::Config(
                    "callback_url is required in production".into(),
                ));
            }
        },
    };

    let payload = StkPushPayload {
        business_short_code: credentials.shortcode.clone(),
        password,
        timestamp,
        transaction_type: "CustomerPayBillOnline".into(),
        amount: request.amount.to_string(),
        party_a: phone.clone(),
        party_b: credentials.shortcode.clone(),
        phone_number: phone,
        callback_url,
        account_reference: request
            .account_reference
            .clone()
            .unwrap_or_else(|| "Pesaraja Payment".into()),
        transaction_desc: request.description.clone(),
    };

    let url = format!(
        "{}/mpesa/stkpush/v1/processrequest",
        credentials.environment.base_url()
    );

    let resp = client
        .post(&url)
        .bearer_auth(&token)
        .json(&payload)
        .send()
        .await?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(parse_daraja_error(&body));
    }

    let result: StkPushResponse = resp.json().await?;

    if result.response_code != "0" {
        return Err(parse_daraja_error(&result.response_description));
    }

    Ok(result)
}

fn normalize_phone(phone: &str) -> Result<String, DarajaError> {
    let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();

    if digits.len() == 9 && digits.starts_with('7') {
        Ok(format!("254{digits}"))
    } else if digits.len() == 10 && digits.starts_with("07") {
        Ok(format!("254{}", &digits[1..]))
    } else if digits.len() == 12 && digits.starts_with("254") {
        Ok(digits)
    } else {
        Err(DarajaError::Other(format!(
            "invalid phone number: {phone}. Expected formats: 0712345678, 712345678, or 254712345678"
        )))
    }
}

fn parse_daraja_error(body: &str) -> DarajaError {
    let lower = body.to_lowercase();

    if lower.contains("not registered") || lower.contains("msisdn") {
        DarajaError::PhoneNotRegistered
    } else if lower.contains("cancelled") || lower.contains("cancelled") {
        DarajaError::StkCancelled
    } else if lower.contains("timeout") || lower.contains("timed out") {
        DarajaError::StkTimeout
    } else if lower.contains("insufficient") || lower.contains("balance") {
        DarajaError::InsufficientFunds
    } else if lower.contains("rate") || lower.contains("limit") {
        DarajaError::RateLimited
    } else {
        DarajaError::UnknownDarajaError(body.to_string())
    }
}
