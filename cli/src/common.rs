use crate::prelude::*;

pub(crate) fn confirm_payment(
    phone: &str,
    amount: u64,
    description: &str,
    skip: bool,
) -> Result<(), DarajaError> {
    println!("  Phone: {phone}");
    println!("  Amount: KES {amount}");
    println!("  Description: {description}");
    if skip {
        return Ok(());
    }
    let confirmed = Confirm::with_theme(&theme())
        .with_prompt("Send payment?")
        .default(false)
        .interact()
        .map_err(|e| DarajaError::Config(format!("prompt cancelled: {e}")))?;
    if !confirmed {
        return Err(DarajaError::Config("payment cancelled".into()));
    }
    Ok(())
}

pub(crate) fn print_dry_run(title: &str, fields: &[(&str, String)]) {
    println!("↯ Dry run: {title}");
    println!();
    for (label, value) in fields {
        println!("  {label}: {value}");
    }
    println!();
    println!("  No request was sent.");
    println!();
}

pub(crate) fn configured_environment() -> Result<Environment, DarajaError> {
    let key = load_sdk_key()?;
    match sdk_key_environment(&key) {
        Some(("sandbox", _)) => Ok(Environment::Sandbox),
        Some(("production", _)) => Ok(Environment::Production),
        _ => Err(DarajaError::Config(
            "SDK key must start with dsk_test_ or dsk_live_".into(),
        )),
    }
}

pub(crate) fn resolve_callback_url(callback_url: Option<String>) -> Result<String, DarajaError> {
    let environment = configured_environment()?;
    let callback = match callback_url {
        Some(url) => url,
        None if environment == Environment::Production => {
            return Err(DarajaError::Config(
                "callback URL is required for production — pass --callback-url".into(),
            ));
        }
        None => {
            println!("  Using sandbox callback URL: {DEFAULT_CALLBACK}");
            DEFAULT_CALLBACK.to_string()
        }
    };
    validate_url("callback URL", &callback, environment)?;
    Ok(callback)
}

pub(crate) fn resolve_async_urls(
    result_url: Option<String>,
    timeout_url: Option<String>,
) -> Result<(String, String), DarajaError> {
    let environment = configured_environment()?;
    if environment == Environment::Production && (result_url.is_none() || timeout_url.is_none()) {
        return Err(DarajaError::Config(
            "result and timeout URLs are required for production — pass --result-url and --timeout-url"
                .into(),
        ));
    }

    let result = result_url.unwrap_or_else(|| DEFAULT_RESULT_URL.to_string());
    let timeout = timeout_url.unwrap_or_else(|| DEFAULT_TIMEOUT_URL.to_string());
    validate_url("result URL", &result, environment)?;
    validate_url("timeout URL", &timeout, environment)?;
    Ok((result, timeout))
}

pub(crate) fn theme() -> ColorfulTheme {
    ColorfulTheme::default()
}

pub(crate) fn payment_store_path() -> Result<PathBuf, DarajaError> {
    Ok(daraja_rs::config::daraja_config_dir()?.join("payments.db"))
}

pub(crate) fn open_payment_store() -> Result<PaymentStore, DarajaError> {
    let path = payment_store_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            DarajaError::Config(format!("create payment store directory: {error}"))
        })?;
    }
    PaymentStore::new(path)
}

pub(crate) fn operation_idempotency_key(
    environment: Environment,
    supplied: Option<String>,
    payment_id: &str,
) -> Result<String, DarajaError> {
    let key = match supplied {
        Some(key) => key,
        None if environment == Environment::Production => {
            return Err(DarajaError::MissingIdempotencyKey);
        }
        None => format!("sandbox:{payment_id}"),
    };
    validate_idempotency_key(&key)?;
    Ok(key)
}

pub(crate) fn persist_payment_error(
    store: &mut PaymentStore,
    payment_id: &str,
    error: DarajaError,
) -> DarajaError {
    let status = if error.is_ambiguous_outcome() {
        PaymentStatus::Unknown
    } else {
        PaymentStatus::Failed
    };
    let detail = format!("request ended with {}", error.code());
    match store.transition(payment_id, status, Some(&detail)) {
        Ok(_) => error,
        Err(persistence_error) => DarajaError::LocalStateUncertain(format!(
            "record {} for {payment_id}: {persistence_error}",
            status.as_str()
        )),
    }
}

pub(crate) fn print_banner() {
    println!();
    println!("  █▀▄ ▄▀█ █▀█ ▄▀█ ░░█ ▄▀█   █▀ █▀▄ █▄▀");
    println!("  █▄▀ █▀█ █▀▄ █▀█ █▄█ █▀█   █▄ █▀▄ █░█");
    println!();
    println!("  Setting up your M-Pesa API key.");
    println!();
}

pub(crate) async fn load_daraja() -> Result<Daraja, DarajaError> {
    if let Some((profile, credentials)) = load_active_profile_context()? {
        validate_credentials_for_environment(profile.environment, &credentials)?;
        validate_shortcode(credentials.shortcode.clone())?;
        return Daraja::from_credentials(credentials, profile.environment);
    }

    let key = load_sdk_key()?;
    unsafe {
        std::env::set_var("DARAJA_KEY", &key);
    }
    Daraja::from_env()
}

pub(crate) async fn load_daraja_with_base_url(
    base_url: Option<&str>,
) -> Result<Daraja, DarajaError> {
    let Some(base_url) = base_url else {
        return load_daraja().await;
    };
    let base_url = validate_loopback_base_url(base_url)?;
    let daraja = load_daraja().await?;
    if daraja.client().environment() != Environment::Sandbox {
        return Err(DarajaError::Config(
            "--base-url is only available with sandbox credentials".into(),
        ));
    }
    daraja.with_test_base_url(base_url)
}

pub(crate) fn validate_loopback_base_url(value: &str) -> Result<String, DarajaError> {
    let url = reqwest::Url::parse(value)
        .map_err(|error| DarajaError::Config(format!("invalid --base-url: {error}")))?;
    let host = url.host_str().unwrap_or_default();
    if url.scheme() != "http"
        || !matches!(host, "localhost" | "127.0.0.1" | "::1" | "[::1]")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || (url.path() != "/" && !url.path().is_empty())
    {
        return Err(DarajaError::Config(
            "--base-url must be an http loopback URL without credentials, query, fragment, or path"
                .into(),
        ));
    }
    Ok(value.trim_end_matches('/').to_string())
}

pub(crate) fn load_shortcode() -> Result<String, DarajaError> {
    if let Some((profile, credentials)) = load_active_profile_context()? {
        validate_credentials_for_environment(profile.environment, &credentials)?;
        return validate_shortcode(credentials.shortcode);
    }

    let key = load_sdk_key()?;
    let key_bytes = config::extract_key_from_sdk_key(&key)?;
    let path = credentials_path()?;
    let encrypted = std::fs::read(&path)
        .map_err(|e| DarajaError::Config(format!("failed to read credentials: {e}")))?;
    let creds = config::decrypt_credentials(&encrypted, &key_bytes)?;
    let environment = if key.starts_with("dsk_test_") {
        Environment::Sandbox
    } else {
        Environment::Production
    };
    validate_credentials_for_environment(environment, &creds)?;
    validate_shortcode(creds.shortcode)
}

pub(crate) const DEFAULT_CALLBACK: &str = "https://example.com/callback";
pub(crate) const DEFAULT_RESULT_URL: &str = "https://example.com/result";
pub(crate) const DEFAULT_TIMEOUT_URL: &str = "https://example.com/timeout";
