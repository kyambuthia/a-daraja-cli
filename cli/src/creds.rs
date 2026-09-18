use crate::prelude::*;

pub(crate) const SANDBOX_PASSKEY: &str =
    "bfb279f9aa9bdbcf158e97dd71a467cd2e0c893059b10f78e6b72ada1ed2c919";

pub(crate) type OptionalInitiatorCredentials =
    (Option<Zeroizing<String>>, Option<Zeroizing<String>>);
pub(crate) fn sdk_key_environment(key: &str) -> Option<(&'static str, &'static str)> {
    if key.starts_with("dsk_test_") {
        Some(("sandbox", "dsk_test_"))
    } else if key.starts_with("dsk_live_") {
        Some(("production", "dsk_live_"))
    } else {
        None
    }
}

pub(crate) fn key_source_label() -> String {
    if std::env::var("DARAJA_KEY")
        .ok()
        .is_some_and(|k| !k.is_empty())
    {
        "DARAJA_KEY environment variable".to_string()
    } else {
        format!(
            "auto-loaded from {}",
            key_path().unwrap_or_default().display()
        )
    }
}

pub(crate) fn load_credentials() -> Result<Credentials, DarajaError> {
    let key = load_sdk_key()?;
    let key_bytes = config::extract_key_from_sdk_key(&key)?;
    let path = credentials_path()?;
    let encrypted = std::fs::read(&path)
        .map_err(|e| DarajaError::Config(format!("failed to read credentials: {e}")))?;
    config::decrypt_credentials(&encrypted, &key_bytes)
}

pub(crate) fn try_load_existing_credentials() -> Option<Credentials> {
    if !credentials_path().ok()?.exists() {
        return None;
    }
    load_credentials().ok()
}

pub(crate) fn save_credentials(
    credentials: &Credentials,
    sdk_key: &str,
) -> Result<(), DarajaError> {
    let key_bytes = config::extract_key_from_sdk_key(sdk_key)?;
    let encrypted = config::encrypt_credentials(credentials, &key_bytes)?;

    let creds_path = credentials_path()?;
    if let Some(parent) = creds_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| DarajaError::Config(format!("failed to create config directory: {e}")))?;
    }
    profiles::atomic_write_private(&creds_path, &encrypted).map_err(|e| {
        DarajaError::Config(format!(
            "failed to write credentials to {}: {e}",
            creds_path.display()
        ))
    })?;

    let key_file = key_path()?;
    profiles::atomic_write_private(&key_file, sdk_key.as_bytes()).map_err(|e| {
        DarajaError::Config(format!(
            "failed to write key to {}: {e}",
            key_file.display()
        ))
    })?;

    Ok(())
}

pub(crate) fn personal_profile(
    env_label: &str,
    credentials: &Credentials,
) -> Result<Profile, DarajaError> {
    Ok(Profile {
        name: "personal".into(),
        profile_type: ProfileType::Personal,
        environment: if env_label == "sandbox" {
            Environment::Sandbox
        } else {
            Environment::Production
        },
        merchant_number: credentials.shortcode.clone(),
        credentials_path: profile_credentials_path("personal")?.display().to_string(),
        initiator_configured: credentials.initiator_name.is_some()
            && credentials.initiator_password.is_some(),
    })
}

pub(crate) fn persist_personal_configuration(
    env_label: &str,
    credentials: &Credentials,
    sdk_key: &str,
) -> Result<(), DarajaError> {
    let profile = personal_profile(env_label, credentials)?;
    save_profile(&profile, credentials, sdk_key)?;
    save_credentials(credentials, sdk_key)
}

pub(crate) fn credentials_path() -> Result<PathBuf, DarajaError> {
    Ok(daraja_rs::config::daraja_config_dir()?.join("credentials.enc"))
}

pub(crate) fn key_path() -> Result<PathBuf, DarajaError> {
    Ok(daraja_rs::config::daraja_config_dir()?.join("key"))
}

/// Load the SDK key from DARAJA_KEY or the platform config directory.
pub(crate) fn load_sdk_key() -> Result<String, DarajaError> {
    if let Ok(key) = std::env::var("DARAJA_KEY")
        && !key.is_empty()
    {
        return Ok(key);
    }
    let path = key_path()?;
    profiles::ensure_private_permissions(&path)?;
    let key = std::fs::read_to_string(&path)
        .map(|s| s.trim().to_string())
        .map_err(|_| {
            DarajaError::Config(format!(
                "No DARAJA_KEY set and no key file at {}. Run `pesaraja init` first.",
                path.display()
            ))
        })?;
    if key.is_empty() {
        return Err(DarajaError::Config(format!(
            "Key file at {} is empty. Run `pesaraja init` first.",
            path.display()
        )));
    }
    Ok(key)
}

pub(crate) fn init_environment_label(env: InitEnvironment) -> &'static str {
    match env {
        InitEnvironment::Sandbox => "sandbox",
        InitEnvironment::Production => "production",
    }
}

pub(crate) fn init_key_prefix(env_label: &str) -> &'static str {
    if env_label == "sandbox" {
        "test"
    } else {
        "live"
    }
}

pub(crate) fn prompt_existing_credentials_action(
    existing: &Credentials,
) -> Result<InitAction, DarajaError> {
    let sdk_key = load_sdk_key()?;
    let (env_name, _) = sdk_key_environment(&sdk_key).unwrap_or(("unknown", "dsk_???"));
    println!();
    println!(
        "  Existing credentials found ({env_name}, shortcode {}).",
        existing.shortcode
    );

    let choice: String = Input::with_theme(&theme())
        .with_prompt("[U]pdate  [O]verwrite  [C]ancel")
        .interact_text()
        .map_err(|e| DarajaError::Config(format!("prompt cancelled: {e}")))?;

    match choice.trim().to_ascii_lowercase().as_str() {
        "u" | "update" => Ok(InitAction::Update(existing.clone())),
        "o" | "overwrite" => Ok(InitAction::Overwrite),
        _ => Ok(InitAction::Cancelled),
    }
}

pub(crate) fn build_initiator_credentials(
    name: Option<String>,
    password: Option<Zeroizing<String>>,
) -> Result<OptionalInitiatorCredentials, DarajaError> {
    match (name, password) {
        (Some(n), Some(p)) => Ok((Some(Zeroizing::new(n)), Some(p))),
        (None, None) => Ok((None, None)),
        _ => Err(DarajaError::Config(
            "initiator requires --initiator-name and DARAJA_INITIATOR_PASSWORD".into(),
        )),
    }
}

pub(crate) fn required_secret_from_env(name: &str) -> Result<Zeroizing<String>, DarajaError> {
    let value = std::env::var(name).map_err(|_| {
        DarajaError::Config(format!(
            "{name} is required for non-interactive init; secrets are never accepted as CLI arguments"
        ))
    })?;
    if value.is_empty() {
        return Err(DarajaError::Config(format!(
            "{name} must not be empty for non-interactive init"
        )));
    }
    Ok(Zeroizing::new(value))
}

pub(crate) fn looks_like_msisdn(shortcode: &str) -> bool {
    let shortcode = shortcode.trim().trim_start_matches('+');
    shortcode.chars().all(|c| c.is_ascii_digit())
        && ((shortcode.starts_with("07") || shortcode.starts_with("01")) && shortcode.len() == 10
            || shortcode.starts_with("254") && shortcode.len() == 12)
}

pub(crate) fn validate_shortcode(shortcode: String) -> Result<String, DarajaError> {
    let shortcode = shortcode.trim().to_string();
    if shortcode.is_empty() {
        return Err(DarajaError::Config("missing --shortcode".into()));
    }
    if looks_like_msisdn(&shortcode) {
        return Err(DarajaError::Config(
            "shortcode looks like a phone number; enter a BusinessShortCode instead".into(),
        ));
    }
    if !shortcode.chars().all(|c| c.is_ascii_digit()) {
        return Err(DarajaError::Config(
            "shortcode must contain digits only".into(),
        ));
    }
    Ok(shortcode)
}

pub(crate) fn validate_credentials_for_environment(
    env: Environment,
    credentials: &Credentials,
) -> Result<(), DarajaError> {
    if matches!(env, Environment::Sandbox) && credentials.passkey.as_str() != SANDBOX_PASSKEY {
        return Err(DarajaError::Config(
            "sandbox passkey does not match the documented sandbox passkey; re-run `pesaraja init` and use the public sandbox passkey".into(),
        ));
    }

    Ok(())
}

pub(crate) fn credentials_from_init_options(
    opts: &InitOptions,
) -> Result<Credentials, DarajaError> {
    let env = opts
        .env
        .ok_or_else(|| DarajaError::Config("missing --env".into()))?;
    let consumer_secret = required_secret_from_env("DARAJA_CONSUMER_SECRET")?;
    let passkey = match env {
        InitEnvironment::Sandbox => Zeroizing::new(SANDBOX_PASSKEY.to_string()),
        InitEnvironment::Production => required_secret_from_env("DARAJA_PASSKEY")?,
    };
    let initiator_password = std::env::var("DARAJA_INITIATOR_PASSWORD")
        .ok()
        .filter(|value| !value.is_empty())
        .map(Zeroizing::new);
    let (initiator_name, initiator_password) =
        build_initiator_credentials(opts.initiator_name.clone(), initiator_password)?;

    Ok(Credentials {
        consumer_key: Zeroizing::new(
            opts.consumer_key
                .clone()
                .ok_or_else(|| DarajaError::Config("missing --consumer-key".into()))?,
        ),
        consumer_secret,
        passkey,
        shortcode: validate_shortcode(
            opts.shortcode
                .clone()
                .ok_or_else(|| DarajaError::Config("missing --shortcode".into()))?,
        )?,
        initiator_name,
        initiator_password,
    })
}

pub(crate) fn prompt_credentials_interactive(
    env_label: &str,
    account_label: &str,
    defaults: Option<&Credentials>,
) -> Result<Credentials, DarajaError> {
    let consumer_key_default = defaults
        .map(|c| c.consumer_key.to_string())
        .unwrap_or_default();
    let consumer_key: String = Input::with_theme(&theme())
        .with_prompt("Consumer Key")
        .default(consumer_key_default)
        .interact_text()
        .map_err(|e| DarajaError::Config(format!("prompt cancelled: {e}")))?;

    let consumer_secret: String = if defaults.is_some() {
        Password::with_theme(&theme())
            .with_prompt("Consumer Secret (Enter to keep existing)")
            .allow_empty_password(true)
            .interact()
            .map_err(|e| DarajaError::Config(format!("prompt cancelled: {e}")))?
    } else {
        Password::with_theme(&theme())
            .with_prompt("Consumer Secret")
            .interact()
            .map_err(|e| DarajaError::Config(format!("prompt cancelled: {e}")))?
    };
    let consumer_secret = if consumer_secret.is_empty() {
        defaults
            .map(|c| c.consumer_secret.to_string())
            .unwrap_or(consumer_secret)
    } else {
        consumer_secret
    };

    let passkey: String = if env_label == "sandbox" {
        println!("  ℹ Sandbox passkey is public and fixed.");
        SANDBOX_PASSKEY.to_string()
    } else if defaults.is_some() {
        Password::with_theme(&theme())
            .with_prompt("Passkey (Enter to keep existing)")
            .allow_empty_password(true)
            .interact()
            .map_err(|e| DarajaError::Config(format!("prompt cancelled: {e}")))?
    } else {
        Password::with_theme(&theme())
            .with_prompt("Passkey")
            .interact()
            .map_err(|e| DarajaError::Config(format!("prompt cancelled: {e}")))?
    };
    let passkey = if passkey.is_empty() {
        defaults.map(|c| c.passkey.to_string()).unwrap_or(passkey)
    } else {
        passkey
    };

    let default_shortcode = defaults.map(|c| c.shortcode.clone()).unwrap_or_else(|| {
        if env_label == "sandbox" {
            "174379".to_string()
        } else {
            String::new()
        }
    });
    let shortcode: String = Input::with_theme(&theme())
        .with_prompt(account_label)
        .default(default_shortcode)
        .interact_text()
        .map_err(|e| DarajaError::Config(format!("prompt cancelled: {e}")))?;
    let shortcode = validate_shortcode(shortcode)?;

    println!();
    println!("  For send / query / balance / refund (optional):");
    println!();

    let had_initiator = defaults.and_then(|c| c.initiator_name.as_ref()).is_some();
    let add_initiator = Confirm::with_theme(&theme())
        .with_prompt("Configure initiator credentials?")
        .default(had_initiator || env_label == "sandbox")
        .interact()
        .map_err(|e| DarajaError::Config(format!("prompt cancelled: {e}")))?;

    let (initiator_name, initiator_password) = if add_initiator {
        let default_name = defaults
            .and_then(|c| c.initiator_name.as_ref())
            .map(|n| n.to_string())
            .unwrap_or_else(|| {
                if env_label == "sandbox" {
                    "apitest".to_string()
                } else {
                    String::new()
                }
            });
        let name: String = Input::with_theme(&theme())
            .with_prompt("Initiator Name")
            .default(default_name)
            .interact_text()
            .map_err(|e| DarajaError::Config(format!("prompt cancelled: {e}")))?;

        let password: String = if defaults.is_some() {
            Password::with_theme(&theme())
                .with_prompt("Initiator Password (Enter to keep existing)")
                .allow_empty_password(true)
                .interact()
                .map_err(|e| DarajaError::Config(format!("prompt cancelled: {e}")))?
        } else {
            Password::with_theme(&theme())
                .with_prompt("Initiator Password")
                .interact()
                .map_err(|e| DarajaError::Config(format!("prompt cancelled: {e}")))?
        };
        let password = if password.is_empty() {
            defaults
                .and_then(|c| c.initiator_password.as_ref())
                .map(|p| p.to_string())
                .unwrap_or(password)
        } else {
            password
        };

        (Some(Zeroizing::new(name)), Some(Zeroizing::new(password)))
    } else {
        (None, None)
    };

    Ok(Credentials {
        consumer_key: Zeroizing::new(consumer_key),
        consumer_secret: Zeroizing::new(consumer_secret),
        passkey: Zeroizing::new(passkey),
        shortcode,
        initiator_name,
        initiator_password,
    })
}

pub(crate) fn print_init_success(env_label: &str, sdk_key: &str) -> Result<(), DarajaError> {
    let key_path = key_path()?;
    println!();
    println!("  ✓ Encrypted credentials saved.");
    println!(
        "  ✓ SDK key saved to {} (owner-read-only)",
        key_path.display()
    );
    println!();
    println!("  Your SDK key:");
    println!();
    println!("    {sdk_key}");
    println!();
    println!("  The key is auto-loaded by all pesaraja commands. No env var needed.");
    println!("  For CI/CD or Docker, set DARAJA_KEY=\"{sdk_key}\"");
    println!();
    if env_label == "sandbox" {
        println!(
            "  Sandbox API certificate saved to {}",
            certificate_path(Environment::Sandbox)?.display()
        );
        println!("  Add production credentials later with:");
        println!("    pesaraja init --env production");
    } else {
        println!("  For send / query / balance / refund in production, save the");
        println!("  Safaricom API certificate from the Daraja portal to:");
        println!(
            "    {}",
            certificate_path(Environment::Production)?.display()
        );
    }
    println!();
    Ok(())
}
