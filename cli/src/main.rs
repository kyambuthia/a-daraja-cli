#![allow(clippy::items_after_test_module)] // Keep private CLI tests beside their helper functions.

mod args;
mod common;
mod creds;
mod input;
mod output;
mod payments;
mod prelude;
mod profiles;
mod receive;
mod simulator;

pub(crate) use prelude::*;

use clap::{Parser, Subcommand, ValueEnum};
use daraja_rs::api::b2c::b2c_payment_with_idempotency_key;
use daraja_rs::api::balance::query_balance;
use daraja_rs::api::c2b::{register_urls, simulate_c2b};
use daraja_rs::api::reversal::reverse_transaction_with_idempotency_key;
use daraja_rs::api::status::query_status;
use daraja_rs::api::stk_push::{stk_push_query, stk_push_with_type_and_idempotency_key};

use daraja_rs::Daraja;
use daraja_rs::auth::TokenManager;
use daraja_rs::certs::{certificate_path, ensure_sandbox_certificate};
use daraja_rs::client::validate_idempotency_key;
use daraja_rs::config::{self, Credentials};
use daraja_rs::error::DarajaError;
use daraja_rs::payments::{NewPayment, PaymentKind, PaymentStatus, PaymentStore};
use daraja_rs::types::{B2cCommand, Environment, StkTransactionType};
use daraja_rs::validation::{
    validate_optional_text, validate_phone, validate_required_text,
    validate_shortcode as validate_merchant_shortcode, validate_url,
};
use daraja_rs::webhook::C2bConfirmation;
use dialoguer::{Confirm, Input, Password, Select, theme::ColorfulTheme};
use input::parse_kes_amount;
use std::path::PathBuf;
use std::process::ExitCode;
use uuid::Uuid;
use zeroize::Zeroizing;

use profiles::{
    Profile, ProfileType, active_profile, load_active_profile_context, load_registry,
    profile_credentials_path, profile_summary, save_profile, select_profile, validate_profile_name,
};

#[cfg(test)]
static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());









#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    let json = cli.json;
    match run(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            output::print_error(&err, json);
            output::exit_code(&err)
        }
    }
}

async fn run(cli: Cli) -> Result<(), DarajaError> {
    let json = cli.json;
    if let Some(profile) = cli.profile.as_deref() {
        validate_profile_name(profile)?;
        unsafe { std::env::set_var("PESARAJA_PROFILE", profile) };
    }

    match cli.command {
        Commands::Setup {
            profile_type,
            name,
            env,
            consumer_key,
            shortcode,
            initiator_name,
            non_interactive,
        } => {
            cmd_setup(
                profile_type,
                name,
                env,
                consumer_key,
                shortcode,
                initiator_name,
                non_interactive,
            )
            .await
        }
        Commands::Use { name } => cmd_use(name),
        Commands::Init {
            env,
            consumer_key,
            shortcode,
            initiator_name,
            non_interactive,
        } => {
            cmd_init(InitOptions {
                env,
                consumer_key,
                shortcode,
                initiator_name,
                non_interactive,
            })
            .await
        }
        Commands::Config => cmd_config(json).await,
        Commands::ExportKey => cmd_export_key().await,
        Commands::Prompt {
            phone,
            amount,
            description,
            reference,
            callback_url,
            payment_type,
            yes,
            dry_run,
            idempotency_key,
            base_url,
        } => {
            cmd_prompt(
                phone,
                amount,
                description,
                reference,
                callback_url,
                payment_type,
                yes,
                dry_run,
                idempotency_key,
                base_url,
                json,
            )
            .await
        }
        Commands::PromptStatus {
            checkout_id,
            base_url,
        } => cmd_prompt_status(checkout_id, base_url, json).await,
        Commands::Payments { command } => cmd_payments(command, json).await,
        Commands::Receive { port } => receive::run(port).await,
        Commands::Simulate {
            phone,
            amount,
            reference,
            shortcode,
        } => cmd_simulate(phone, amount, reference, shortcode, json).await,
        Commands::Send {
            to,
            amount,
            remarks,
            occasion,
            payment_type,
            result_url,
            timeout_url,
            yes,
            dry_run,
            idempotency_key,
        } => {
            cmd_send(
                to,
                amount,
                remarks,
                occasion,
                payment_type,
                result_url,
                timeout_url,
                yes,
                dry_run,
                idempotency_key,
                json,
            )
            .await
        }
        Commands::Query {
            id,
            remarks,
            occasion,
            result_url,
            timeout_url,
        } => cmd_query(id, remarks, occasion, result_url, timeout_url, json).await,
        Commands::Balance {
            result_url,
            timeout_url,
        } => cmd_balance(result_url, timeout_url, json).await,
        Commands::Refund {
            id,
            amount,
            receiver,
            receiver_type,
            remarks,
            occasion,
            result_url,
            timeout_url,
            dry_run,
            yes,
            idempotency_key,
        } => {
            let amount = parse_kes_amount(&amount)?;
            cmd_refund(
                id,
                amount,
                receiver,
                receiver_type,
                remarks,
                occasion,
                result_url,
                timeout_url,
                dry_run,
                yes,
                idempotency_key,
                json,
            )
            .await
        }
        Commands::Register {
            shortcode,
            response_type,
            confirm_url,
            validation_url,
        } => cmd_register(shortcode, response_type, confirm_url, validation_url, json).await,
        Commands::Webhook { url, endpoint } => cmd_webhook(url, endpoint, json).await,
        Commands::Simulator { port, scenario } => simulator::run(port, scenario).await,
        Commands::Doctor => cmd_doctor(json).await,
    }
}


async fn cmd_init(opts: InitOptions) -> Result<(), DarajaError> {
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
async fn cmd_setup(
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

fn cmd_use(name: String) -> Result<(), DarajaError> {
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

async fn cmd_config(json: bool) -> Result<(), DarajaError> {
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

async fn cmd_export_key() -> Result<(), DarajaError> {
    let key = load_sdk_key()?;
    print!("{key}");
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use daraja_rs::config::generate_sdk_key;
    use zeroize::Zeroizing;

    fn temp_home() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp dir");
        unsafe { std::env::set_var("HOME", dir.path()) };
        dir
    }

    fn test_credentials(shortcode: &str) -> Credentials {
        Credentials {
            consumer_key: Zeroizing::new("ck".into()),
            consumer_secret: Zeroizing::new("cs".into()),
            passkey: Zeroizing::new("pk".into()),
            shortcode: shortcode.into(),
            initiator_name: Some(Zeroizing::new("apitest".into())),
            initiator_password: Some(Zeroizing::new("pass".into())),
        }
    }

    fn sandbox_credentials(shortcode: &str, passkey: &str) -> Credentials {
        Credentials {
            consumer_key: Zeroizing::new("ck".into()),
            consumer_secret: Zeroizing::new("cs".into()),
            passkey: Zeroizing::new(passkey.into()),
            shortcode: shortcode.into(),
            initiator_name: Some(Zeroizing::new("apitest".into())),
            initiator_password: Some(Zeroizing::new("pass".into())),
        }
    }

    fn seed_profile_home(shortcode: &str, passkey: &str) -> tempfile::TempDir {
        let home = temp_home();
        let sdk_key = generate_sdk_key("test");
        let config_dir = home.path().join(".config/daraja");
        std::fs::create_dir_all(&config_dir).expect("create config dir");
        profiles::atomic_write_private(&config_dir.join("key"), sdk_key.as_bytes())
            .expect("write key");
        let creds = sandbox_credentials(shortcode, passkey);
        let profile = Profile {
            name: "personal".into(),
            profile_type: ProfileType::Personal,
            environment: Environment::Sandbox,
            merchant_number: shortcode.into(),
            credentials_path: profile_credentials_path("personal")
                .expect("profile credentials path")
                .display()
                .to_string(),
            initiator_configured: true,
        };

        save_profile(&profile, &creds, &sdk_key).expect("seed profile");
        home
    }

    #[test]
    fn personal_configuration_update_refreshes_profile_credentials() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let _home = temp_home();
        let sdk_key = generate_sdk_key("test");

        let first = test_credentials("174379");
        persist_personal_configuration("sandbox", &first, &sdk_key).expect("initial persist");

        let (active, loaded) = load_active_profile_context()
            .expect("load active context")
            .expect("active profile");
        assert_eq!(active.merchant_number, "174379");
        assert_eq!(loaded.shortcode, "174379");

        let updated = test_credentials("254743121299");
        persist_personal_configuration("sandbox", &updated, &sdk_key).expect("updated persist");

        let (active, loaded) = load_active_profile_context()
            .expect("reload active context")
            .expect("active profile after update");
        assert_eq!(active.merchant_number, "254743121299");
        assert_eq!(loaded.shortcode, "254743121299");
    }

    #[test]
    fn sandbox_async_urls_use_test_defaults_and_validate_them() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let home = temp_home();
        let config_dir = home.path().join(".config/daraja");
        std::fs::create_dir_all(&config_dir).expect("create config dir");
        profiles::atomic_write_private(
            &config_dir.join("key"),
            generate_sdk_key("test").as_bytes(),
        )
        .expect("write key");
        unsafe { std::env::remove_var("DARAJA_KEY") };

        let (result, timeout) = resolve_async_urls(None, None).expect("sandbox defaults");
        assert_eq!(result, DEFAULT_RESULT_URL);
        assert_eq!(timeout, DEFAULT_TIMEOUT_URL);
    }

    #[test]
    fn production_async_urls_require_explicit_values() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let home = temp_home();
        let config_dir = home.path().join(".config/daraja");
        std::fs::create_dir_all(&config_dir).expect("create config dir");
        profiles::atomic_write_private(
            &config_dir.join("key"),
            generate_sdk_key("live").as_bytes(),
        )
        .expect("write key");
        unsafe { std::env::remove_var("DARAJA_KEY") };

        let error = resolve_async_urls(None, None).expect_err("production defaults must fail");
        assert!(
            error
                .to_string()
                .contains("result and timeout URLs are required")
        );
    }

    #[test]
    fn callback_urls_are_validated_before_dry_run() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let home = temp_home();
        let config_dir = home.path().join(".config/daraja");
        std::fs::create_dir_all(&config_dir).expect("create config dir");
        profiles::atomic_write_private(
            &config_dir.join("key"),
            generate_sdk_key("test").as_bytes(),
        )
        .expect("write key");
        unsafe { std::env::remove_var("DARAJA_KEY") };

        let error = resolve_callback_url(Some("not-a-url".into()))
            .expect_err("invalid callback URL must fail");
        assert!(error.to_string().contains("callback URL"));
    }

    #[test]
    fn init_options_reject_phone_number_shortcodes() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let opts = InitOptions {
            env: Some(InitEnvironment::Sandbox),
            consumer_key: Some("ck".into()),
            shortcode: Some("0743121299".into()),
            initiator_name: None,
            non_interactive: true,
        };
        unsafe { std::env::set_var("DARAJA_CONSUMER_SECRET", "cs") };

        let err = match credentials_from_init_options(&opts) {
            Ok(_) => panic!("phone number should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("BusinessShortCode"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn init_options_accept_business_shortcodes() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let opts = InitOptions {
            env: Some(InitEnvironment::Sandbox),
            consumer_key: Some("ck".into()),
            shortcode: Some("174379".into()),
            initiator_name: None,
            non_interactive: true,
        };
        unsafe { std::env::set_var("DARAJA_CONSUMER_SECRET", "cs") };

        let creds = credentials_from_init_options(&opts).expect("business shortcode should pass");
        assert_eq!(creds.shortcode, "174379");
    }

    #[test]
    fn non_interactive_sandbox_init_uses_public_passkey() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let opts = InitOptions {
            env: Some(InitEnvironment::Sandbox),
            consumer_key: Some("ck".into()),
            shortcode: Some("174379".into()),
            initiator_name: None,
            non_interactive: true,
        };
        unsafe { std::env::set_var("DARAJA_CONSUMER_SECRET", "cs") };

        let creds = credentials_from_init_options(&opts)
            .expect("sandbox init should use the public passkey");
        assert_eq!(creds.passkey.as_str(), SANDBOX_PASSKEY);
    }

    #[test]
    fn sandbox_credentials_must_use_documented_passkey() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let creds = sandbox_credentials("174379", "wrong-passkey");
        let err = match validate_credentials_for_environment(Environment::Sandbox, &creds) {
            Ok(_) => panic!("sandbox passkey should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("documented sandbox passkey"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn sandbox_credentials_accept_documented_passkey() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let creds = sandbox_credentials("174379", SANDBOX_PASSKEY);
        validate_credentials_for_environment(Environment::Sandbox, &creds)
            .expect("documented sandbox passkey should be accepted");
    }

    #[test]
    fn validate_shortcode_rejects_msisdn_variants() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        for candidate in ["0743121299", "0112345678", "254743121299", "+254743121299"] {
            let err = match validate_shortcode(candidate.to_string()) {
                Ok(_) => panic!("mobile numbers should be rejected"),
                Err(err) => err,
            };
            assert!(
                err.to_string().contains("BusinessShortCode"),
                "unexpected error for {candidate}: {err}"
            );
        }
    }

    #[test]
    fn validate_shortcode_trims_and_accepts_business_shortcode() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let shortcode = validate_shortcode(" 174379 ".into()).expect("shortcode should pass");
        assert_eq!(shortcode, "174379");
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // Serializes process-wide HOME mutations for this test.
    async fn load_daraja_rejects_wrong_sandbox_passkey() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let _home = seed_profile_home("174379", "wrong-passkey");

        let err = match load_daraja().await {
            Ok(_) => panic!("wrong sandbox passkey should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("documented sandbox passkey"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn load_shortcode_reads_active_profile_shortcode() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let _home = seed_profile_home("174379", SANDBOX_PASSKEY);

        let shortcode = load_shortcode().expect("active profile shortcode");
        assert_eq!(shortcode, "174379");
    }

    #[test]
    fn base_url_accepts_loopback_http_and_removes_trailing_slash() {
        assert_eq!(
            validate_loopback_base_url("http://127.0.0.1:8787/").expect("loopback URL should pass"),
            "http://127.0.0.1:8787"
        );
        assert!(validate_loopback_base_url("http://localhost:8787").is_ok());
        assert!(validate_loopback_base_url("http://[::1]:8787").is_ok());
    }

    #[test]
    fn base_url_rejects_non_loopback_and_unsafe_urls() {
        for candidate in [
            "https://127.0.0.1:8787",
            "http://example.com:8787",
            "http://user:pass@127.0.0.1:8787",
            "http://127.0.0.1:8787/simulator",
            "http://127.0.0.1:8787/?token=secret",
        ] {
            assert!(
                validate_loopback_base_url(candidate).is_err(),
                "unsafe base URL should be rejected: {candidate}"
            );
        }
    }
}

#[allow(clippy::too_many_arguments)] // Mirrors the user-facing Clap command fields.
async fn cmd_prompt(
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

async fn cmd_prompt_status(
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

async fn cmd_payments(command: PaymentCommands, json: bool) -> Result<(), DarajaError> {
    payments::run(command, json).await
}

async fn cmd_simulate(
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
async fn cmd_send(
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

async fn cmd_query(
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

async fn cmd_balance(
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
async fn cmd_refund(
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

fn sample_c2b_confirmation() -> C2bConfirmation {
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

async fn cmd_webhook(url: String, endpoint: Option<String>, json: bool) -> Result<(), DarajaError> {
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

async fn cmd_register(
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

struct CheckItem {
    ok: bool,
    message: String,
    hint: Option<String>,
}

fn print_check(item: &CheckItem) {
    let mark = if item.ok { "✓" } else { "✗" };
    println!("{mark} {}", item.message);
    if let Some(hint) = &item.hint {
        println!("  {hint}");
    }
}

async fn cmd_doctor(json: bool) -> Result<(), DarajaError> {
    let mut checks: Vec<CheckItem> = Vec::new();

    let creds_path = credentials_path()?;
    let creds_exist = creds_path.exists();
    checks.push(CheckItem {
        ok: creds_exist,
        message: if creds_exist {
            format!("Credentials encrypted at rest ({})", creds_path.display())
        } else {
            "Credentials file not found".to_string()
        },
        hint: if creds_exist {
            None
        } else {
            Some("Run `pesaraja init` to set up.".to_string())
        },
    });

    if creds_exist {
        let permissions_ok = profiles::ensure_private_permissions(&creds_path).is_ok();
        checks.push(CheckItem {
            ok: permissions_ok,
            message: if permissions_ok {
                "Credentials file permissions are private".into()
            } else {
                "Credentials file is accessible to other users".into()
            },
            hint: if permissions_ok {
                None
            } else {
                Some(format!("Run `chmod 600 {}`.", creds_path.display()))
            },
        });
    }

    let daraja_key = load_sdk_key().ok();
    let key_set = daraja_key.is_some();
    let key_from_env = std::env::var("DARAJA_KEY").ok().is_some();

    checks.push(CheckItem {
        ok: key_set,
        message: if key_set {
            if key_from_env {
                "DARAJA_KEY is set (env var)".to_string()
            } else {
                format!("SDK key loaded from {}", key_path()?.display())
            }
        } else {
            "DARAJA_KEY is not set".to_string()
        },
        hint: if key_set {
            None
        } else {
            Some("Run `pesaraja init` to set up.".to_string())
        },
    });

    if !key_from_env {
        let path = key_path()?;
        if path.exists() {
            let permissions_ok = profiles::ensure_private_permissions(&path).is_ok();
            checks.push(CheckItem {
                ok: permissions_ok,
                message: if permissions_ok {
                    "SDK key file permissions are private".into()
                } else {
                    "SDK key file is accessible to other users".into()
                },
                hint: if permissions_ok {
                    None
                } else {
                    Some(format!("Run `chmod 600 {}`.", path.display()))
                },
            });
        }
    }

    let is_sandbox = daraja_key
        .as_deref()
        .map(|k| k.starts_with("dsk_test_"))
        .unwrap_or(false);
    let is_production = daraja_key
        .as_deref()
        .map(|k| k.starts_with("dsk_live_"))
        .unwrap_or(false);

    if let Some(key) = &daraja_key {
        if !is_sandbox && !is_production {
            checks.push(CheckItem {
                ok: false,
                message: "SDK key has an invalid prefix (expected dsk_test_ or dsk_live_)".into(),
                hint: Some("Re-run `pesaraja init` to generate a valid key.".into()),
            });
        } else if creds_exist {
            match config::extract_key_from_sdk_key(key) {
                Ok(key_bytes) => {
                    let encrypted = std::fs::read(&creds_path).map_err(|e| {
                        DarajaError::Config(format!("failed to read credentials: {e}"))
                    })?;
                    match config::decrypt_credentials(&encrypted, &key_bytes) {
                        Ok(creds) => {
                            let env = if is_sandbox {
                                Environment::Sandbox
                            } else {
                                Environment::Production
                            };
                            let env_name = if is_sandbox { "Sandbox" } else { "Production" };

                            let mut token_manager = TokenManager::new(
                                creds.consumer_key.to_string(),
                                creds.consumer_secret.to_string(),
                                env,
                            );
                            match token_manager.get_token().await {
                                Ok(token) => {
                                    let expires_in =
                                        (token.expires_at - chrono::Utc::now()).num_seconds();
                                    let expires_in = expires_in.max(0);
                                    checks.push(CheckItem {
                                        ok: true,
                                        message: format!(
                                            "{env_name} OAuth token: valid (expires in {expires_in}s)"
                                        ),
                                        hint: None,
                                    });
                                }
                                Err(err) => {
                                    checks.push(CheckItem {
                                        ok: false,
                                        message: format!("{env_name} OAuth failed: {err}"),
                                        hint: Some(
                                            "Check your consumer key and secret in `pesaraja init`."
                                                .into(),
                                        ),
                                    });
                                }
                            }

                            let has_initiator = creds.initiator_name.is_some()
                                && creds.initiator_password.is_some();
                            if has_initiator {
                                checks.push(CheckItem {
                                    ok: true,
                                    message: "Initiator credentials configured".into(),
                                    hint: None,
                                });
                            } else {
                                checks.push(CheckItem {
                                    ok: false,
                                    message: "Initiator credentials not configured".into(),
                                    hint: Some(
                                        "Required for send, query, balance, and refund. \
                                         Re-run `pesaraja init` and add initiator details."
                                            .into(),
                                    ),
                                });
                            }

                            if is_sandbox {
                                if let Err(err) = validate_credentials_for_environment(
                                    Environment::Sandbox,
                                    &creds,
                                ) {
                                    checks.push(CheckItem {
                                        ok: false,
                                        message: format!("Sandbox passkey mismatch: {err}"),
                                        hint: Some(
                                            "Re-run `pesaraja init` and use the public sandbox passkey."
                                                .into(),
                                        ),
                                    });
                                } else {
                                    checks.push(CheckItem {
                                        ok: true,
                                        message: "Sandbox passkey matches the documented value"
                                            .into(),
                                        hint: None,
                                    });
                                }
                            }

                            if is_sandbox {
                                checks.push(CheckItem {
                                    ok: false,
                                    message: "send: not available in sandbox (needs Go Live)"
                                        .into(),
                                    hint: None,
                                });
                            } else if has_initiator {
                                checks.push(CheckItem {
                                    ok: true,
                                    message: "send: credentials ready".into(),
                                    hint: None,
                                });
                            }
                        }
                        Err(err) => {
                            checks.push(CheckItem {
                                ok: false,
                                message: format!("Failed to decrypt credentials: {err}"),
                                hint: Some(
                                    "Your DARAJA_KEY may not match the encrypted file.".into(),
                                ),
                            });
                        }
                    }
                }
                Err(err) => {
                    checks.push(CheckItem {
                        ok: false,
                        message: format!("Invalid SDK key format: {err}"),
                        hint: None,
                    });
                }
            }
        }
    }

    if is_sandbox {
        checks.push(CheckItem {
            ok: true,
            message: "Using sandbox environment (dsk_test_)".into(),
            hint: None,
        });

        let sandbox_cert_path = certificate_path(Environment::Sandbox)?;
        let sandbox_cert_ok = sandbox_cert_path.exists();
        checks.push(CheckItem {
            ok: sandbox_cert_ok,
            message: if sandbox_cert_ok {
                "Sandbox API certificate found".into()
            } else {
                "Sandbox API certificate not found".into()
            },
            hint: if sandbox_cert_ok {
                None
            } else {
                Some(format!(
                    "Run `pesaraja init` to download {}.",
                    sandbox_cert_path.display()
                ))
            },
        });

        checks.push(CheckItem {
            ok: false,
            message: "Production credentials not configured".into(),
            hint: Some("Run `pesaraja init --env production` to set up.".into()),
        });
    } else if is_production {
        checks.push(CheckItem {
            ok: true,
            message: "Production environment configured (dsk_live_)".into(),
            hint: None,
        });

        let production_cert_path = certificate_path(Environment::Production)?;
        let cert_ok = production_cert_path.exists();
        checks.push(CheckItem {
            ok: cert_ok,
            message: if cert_ok {
                "Production API certificate found".into()
            } else {
                "Production API certificate not found".into()
            },
            hint: if cert_ok {
                None
            } else {
                Some(format!(
                    "Download from the Daraja portal and save to {} (or set DARAJA_CERT_PATH).",
                    production_cert_path.display()
                ))
            },
        });
    }

    checks.push(CheckItem {
        ok: true,
        message: format!("Default callback URL: {DEFAULT_CALLBACK} (sandbox placeholder)"),
        hint: Some(
            "For production, expose a public HTTPS endpoint (e.g. ngrok) and pass --callback-url."
                .into(),
        ),
    });

    let critical_failed = !creds_exist
        || !key_set
        || checks.iter().any(|c| {
            c.message.contains("OAuth failed")
                || c.message.contains("Failed to decrypt")
                || c.message.contains("invalid prefix")
                || c.message.contains("Invalid SDK key")
                || c.message.contains("Sandbox passkey mismatch")
        });

    if json {
        let output = serde_json::json!({
            "critical_failed": critical_failed,
            "checks": checks.iter().map(|check| serde_json::json!({
                "ok": check.ok,
                "message": check.message,
                "hint": check.hint,
            })).collect::<Vec<_>>(),
        });
        output::print_json(&output)?;
    } else {
        println!();
        for check in &checks {
            print_check(check);
        }
        println!();
    }

    if critical_failed {
        return Err(DarajaError::Config(
            "critical checks failed — review the report above".into(),
        ));
    }

    Ok(())
}
