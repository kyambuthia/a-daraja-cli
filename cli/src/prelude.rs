//! Shared imports for CLI modules.
//!
//! Imported with `use crate::prelude::*;`. Glob imports (and re-exports)
//! do not trigger `unused_imports`, which keeps the per-command modules
//! warning-clean under `clippy -D warnings`.

pub(crate) use crate::args::{
    Cli, Commands, InitAction, InitEnvironment, InitOptions, PaymentCommands, PromptType, SendType,
    SetupProfileType,
};
pub(crate) use crate::common::{
    DEFAULT_CALLBACK, DEFAULT_RESULT_URL, DEFAULT_TIMEOUT_URL, configured_environment, confirm_payment, load_daraja,
    load_daraja_with_base_url, load_shortcode, open_payment_store, operation_idempotency_key,
    persist_payment_error, print_banner, print_dry_run, resolve_async_urls, resolve_callback_url,
    theme, validate_loopback_base_url,
};
pub(crate) use crate::creds::{
    credentials_from_init_options, credentials_path, init_environment_label, init_key_prefix,
    key_path, key_source_label, load_credentials, load_sdk_key, persist_personal_configuration,
    print_init_success, prompt_credentials_interactive, prompt_existing_credentials_action,
    sdk_key_environment, try_load_existing_credentials, validate_credentials_for_environment,
    validate_shortcode, SANDBOX_PASSKEY,
};
pub(crate) use crate::input::parse_kes_amount;
pub(crate) use crate::output;
pub(crate) use crate::profiles;
pub(crate) use crate::profiles::{
    Profile, ProfileType, active_profile, load_active_profile_context, load_registry,
    profile_credentials_path, profile_summary, save_profile, select_profile, validate_profile_name,
};
pub(crate) use crate::simulator;
pub(crate) use clap::{Parser, Subcommand, ValueEnum};
pub(crate) use daraja_rs::Daraja;
pub(crate) use daraja_rs::api::b2c::b2c_payment_with_idempotency_key;
pub(crate) use daraja_rs::api::balance::query_balance;
pub(crate) use daraja_rs::api::c2b::{register_urls, simulate_c2b};
pub(crate) use daraja_rs::api::reversal::reverse_transaction_with_idempotency_key;
pub(crate) use daraja_rs::api::status::query_status;
pub(crate) use daraja_rs::api::stk_push::{stk_push_query, stk_push_with_type_and_idempotency_key};
pub(crate) use daraja_rs::auth::TokenManager;
pub(crate) use daraja_rs::certs::{certificate_path, ensure_sandbox_certificate};
pub(crate) use daraja_rs::client::validate_idempotency_key;
pub(crate) use daraja_rs::config::{self, Credentials};
pub(crate) use daraja_rs::error::DarajaError;
pub(crate) use daraja_rs::payments::{NewPayment, PaymentKind, PaymentStatus, PaymentStore};
pub(crate) use daraja_rs::types::{B2cCommand, Environment, StkTransactionType};
pub(crate) use daraja_rs::validation::{
    validate_optional_text, validate_phone, validate_required_text,
    validate_shortcode as validate_merchant_shortcode, validate_url,
};
pub(crate) use daraja_rs::webhook::C2bConfirmation;
pub(crate) use dialoguer::{Confirm, Input, Password, Select, theme::ColorfulTheme};
pub(crate) use std::path::PathBuf;
pub(crate) use std::process::ExitCode;
pub(crate) use uuid::Uuid;
pub(crate) use zeroize::Zeroizing;
