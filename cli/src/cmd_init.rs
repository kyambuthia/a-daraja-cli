use crate::prelude::*;

pub(crate) async fn cmd_init(opts: InitOptions) -> Result<(), DarajaError> {
    if opts.non_interactive && !opts.has_all_required() {
        return Err(DarajaError::Config(
            "non-interactive init requires --env, --consumer-key, --shortcode, \
             DARAJA_CONSUMER_SECRET, and DARAJA_PASSKEY for production"
                .into(),
        ));
    }

    if opts.has_all_required() {
        let env_label = init_environment_label(opts.env.expect("checked above"));
        let credentials = credentials_from_init_options(&opts)?;
        let sdk_key = config::generate_sdk_key(init_key_prefix(env_label));
        persist_personal_configuration(env_label, &credentials, &sdk_key)?;
        if env_label == "sandbox" {
            ensure_sandbox_certificate().await?;
        }
        print_init_success(env_label, &sdk_key)?;
        return Ok(());
    }

    let existing = try_load_existing_credentials();
    let action = if let Some(ref creds) = existing {
        prompt_existing_credentials_action(creds)?
    } else {
        InitAction::Fresh
    };

    if matches!(action, InitAction::Cancelled) {
        println!("  Cancelled.");
        return Ok(());
    }

    print_banner();

    let (env_label, sdk_key) = match action {
        InitAction::Update(ref creds) => {
            let key = load_sdk_key()?;
            let env_label = sdk_key_environment(&key)
                .map(|(name, _)| name)
                .unwrap_or("sandbox");
            let credentials =
                prompt_credentials_interactive(env_label, "PayBill / Till shortcode", Some(creds))?;
            persist_personal_configuration(env_label, &credentials, &key)?;
            if env_label == "sandbox" {
                ensure_sandbox_certificate().await?;
            }
            print_init_success(env_label, &key)?;
            return Ok(());
        }
        InitAction::Overwrite | InitAction::Fresh => {
            let env_label = match opts.env {
                Some(env) => init_environment_label(env),
                None => {
                    let options = ["sandbox", "production"];
                    let selection = Select::with_theme(&theme())
                        .with_prompt("Environment")
                        .default(0)
                        .items(&options)
                        .interact()
                        .map_err(|e| DarajaError::Config(format!("prompt cancelled: {e}")))?;
                    options[selection]
                }
            };
            let key_prefix = init_key_prefix(env_label);
            let sdk_key = config::generate_sdk_key(key_prefix);
            (env_label, sdk_key)
        }
        InitAction::Cancelled => unreachable!(),
    };

    let credentials = prompt_credentials_interactive(env_label, "PayBill / Till shortcode", None)?;
    persist_personal_configuration(env_label, &credentials, &sdk_key)?;
    if env_label == "sandbox" {
        ensure_sandbox_certificate().await?;
    }
    print_init_success(env_label, &sdk_key)?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn cmd_setup(
    profile_type: SetupProfileType,
    name: Option<String>,
    env: Option<InitEnvironment>,
    consumer_key: Option<String>,
    shortcode: Option<String>,
    initiator_name: Option<String>,
    non_interactive: bool,
) -> Result<(), DarajaError> {
    let sdk_key = load_sdk_key()?;
    let current_env = sdk_key_environment(&sdk_key)
        .map(|(environment, _)| environment)
        .ok_or_else(|| DarajaError::Config("SDK key has an unknown environment prefix".into()))?;
    let env_label = env.map(init_environment_label).unwrap_or(current_env);
    if env_label != current_env {
        return Err(DarajaError::Config(format!(
            "profile environment '{env_label}' does not match the active SDK key environment '{current_env}'; initialize that environment first"
        )));
    }

    let name = match name {
        Some(name) => name,
        None => Input::with_theme(&theme())
            .with_prompt("Profile name")
            .default(profile_type.to_profile_type().as_str().to_string())
            .interact_text()
            .map_err(|e| DarajaError::Config(format!("prompt cancelled: {e}")))?,
    };
    validate_profile_name(&name)?;
    if name == "personal" {
        return Err(DarajaError::Config(
            "merchant setup cannot replace the personal profile; choose another --name".into(),
        ));
    }

    let options = InitOptions {
        env: Some(if env_label == "sandbox" {
            InitEnvironment::Sandbox
        } else {
            InitEnvironment::Production
        }),
        consumer_key,
        shortcode,
        initiator_name,
        non_interactive,
    };
    let credentials = if options.has_all_required() {
        credentials_from_init_options(&options)?
    } else if non_interactive {
        return Err(DarajaError::Config(
            "non-interactive setup requires --consumer-key, --shortcode, DARAJA_CONSUMER_SECRET, and DARAJA_PASSKEY for production"
                .into(),
        ));
    } else {
        prompt_credentials_interactive(env_label, profile_type.account_label(), None)?
    };
    let environment = if env_label == "sandbox" {
        Environment::Sandbox
    } else {
        Environment::Production
    };
    validate_credentials_for_environment(environment, &credentials)?;
    let profile = Profile {
        name: name.clone(),
        profile_type: profile_type.to_profile_type(),
        environment,
        merchant_number: credentials.shortcode.clone(),
        credentials_path: profile_credentials_path(&name)?.display().to_string(),
        initiator_configured: credentials.initiator_name.is_some()
            && credentials.initiator_password.is_some(),
    };
    save_profile(&profile, &credentials, &sdk_key)?;

    println!("✓ Profile '{name}' configured and activated");
    println!("  Type: {}", profile.profile_type.as_str());
    println!("  Environment: {env_label}");
    println!("  Merchant number: {}", profile.merchant_number);
    Ok(())
}

pub(crate) fn cmd_use(name: String) -> Result<(), DarajaError> {
    select_profile(&name)?;
    let registry = load_registry()?;
    let profile = active_profile(&registry)
        .ok_or_else(|| DarajaError::Config(format!("profile '{name}' was not found")))?;
    println!("✓ Active profile: {}", profile.name);
    println!("  Type: {}", profile.profile_type.as_str());
    println!(
        "  Environment: {}",
        match profile.environment {
            Environment::Sandbox => "sandbox",
            Environment::Production => "production",
        }
    );
    println!("  Merchant number: {}", profile.merchant_number);
    Ok(())
}

pub(crate) async fn cmd_config(json: bool) -> Result<(), DarajaError> {
    let creds_path = credentials_path()?;
    let key = load_sdk_key()?;
    let (env_name, env_prefix) = sdk_key_environment(&key).unwrap_or(("unknown", "unknown"));
    let registry = load_registry().unwrap_or_default();

    if !registry.profiles.is_empty() {
        let selected_profile = active_profile(&registry).map(|profile| profile.name.clone());
        if json {
            let output = serde_json::json!({
                "active_profile": selected_profile,
                "profiles": registry.profiles.iter().map(profile_summary).collect::<Vec<_>>(),
                "sdk_key_source": key_source_label(),
            });
            output::print_json(&output)?;
            return Ok(());
        }

        println!();
        if let Some(profile) = active_profile(&registry) {
            println!(
                "  Active profile: {} ({}, {})",
                profile.name,
                profile.profile_type.as_str(),
                profile.merchant_number
            );
        } else {
            println!("  Active profile: —");
        }
        println!();
        println!("  Profiles:");
        for profile in &registry.profiles {
            let active = registry
                .active_profile
                .as_deref()
                .is_some_and(|name| name == profile.name)
                || selected_profile.as_deref() == Some(profile.name.as_str());
            let marker = if active { "[active]" } else { "" };
            println!(
                "  - {} {} {} {} {}",
                profile.name,
                marker,
                profile.profile_type.as_str(),
                profile.merchant_number,
                match profile.environment {
                    Environment::Sandbox => "sandbox",
                    Environment::Production => "production",
                }
            );
        }
        println!("  Key source: {}", key_source_label());
        println!();
        return Ok(());
    }

    if json {
        let credentials = if creds_path.exists() {
            match load_credentials() {
                Ok(creds) => serde_json::json!({
                    "status": "ok",
                    "shortcode": creds.shortcode,
                    "initiator_configured": creds.initiator_name.is_some() && creds.initiator_password.is_some(),
                }),
                Err(err) => serde_json::json!({
                    "status": "unreadable",
                    "error": err.to_string(),
                }),
            }
        } else {
            serde_json::json!({
                "status": "missing",
            })
        };

        let output = serde_json::json!({
            "environment": env_name,
            "prefix": env_prefix,
            "credentials_path": creds_path.display().to_string(),
            "sdk_key_source": key_source_label(),
            "credentials": credentials,
        });
        output::print_json(&output)?;
        return Ok(());
    }

    println!();
    println!("  Environment: {env_name} ({env_prefix})");

    if creds_path.exists() {
        match load_credentials() {
            Ok(creds) => {
                println!("  Shortcode: {}", creds.shortcode);
                if let Some(name) = creds.initiator_name.as_ref() {
                    println!("  Initiator: {} (configured)", name.as_str());
                } else {
                    println!("  Initiator: not configured");
                }
                println!("  Credentials: encrypted ({})", creds_path.display());
            }
            Err(err) => {
                println!("  Credentials: encrypted ({})", creds_path.display());
                println!("  Warning: could not decrypt credentials ({err})");
            }
        }
    } else {
        println!("  Credentials: not found");
        println!("  Shortcode: —");
        println!("  Initiator: not configured");
    }

    println!("  Key source: {}", key_source_label());
    println!();

    Ok(())
}

pub(crate) async fn cmd_export_key() -> Result<(), DarajaError> {
    let key = load_sdk_key()?;
    print!("{key}");
    Ok(())
}
