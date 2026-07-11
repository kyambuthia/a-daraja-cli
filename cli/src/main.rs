mod receive;
mod profiles;

use clap::{Parser, Subcommand, ValueEnum};
use daraja_rs::api::balance::query_balance;
use daraja_rs::api::b2c::b2c_payment;
use daraja_rs::api::c2b::{register_urls, simulate_c2b};
use daraja_rs::api::stk_push::{stk_push_query, stk_push_with_type};
use daraja_rs::api::reversal::reverse_transaction;
use daraja_rs::api::status::query_status;

use daraja_rs::auth::TokenManager;
use daraja_rs::config::{self, Credentials};
use daraja_rs::error::DarajaError;
use daraja_rs::types::{B2cCommand, Environment, StkTransactionType};
use daraja_rs::webhook::C2bConfirmation;
use daraja_rs::certs::ensure_sandbox_certificate;
use daraja_rs::Daraja;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Password, Select};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::ExitCode;
use zeroize::Zeroizing;

use profiles::{
    active_profile, load_active_profile_context, load_registry, profile_credentials_path,
    profile_summary, save_profile, Profile, ProfileType,
};

#[cfg(test)]
static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

const DEFAULT_CALLBACK: &str = "https://example.com/callback";
const DEFAULT_RESULT_URL: &str = "https://example.com/result";
const DEFAULT_TIMEOUT_URL: &str = "https://example.com/timeout";
const SANDBOX_PASSKEY: &str =
    "bfb279f9aa9bdbcf158e97dd71a467cd2e0c893059b10f78e6b72ada1ed2c919";

#[derive(Parser)]
#[command(
    name = "pesaraja",
    version,
    about = "M-Pesa payments — prompt, receive, and send money",
    long_about = "Collect payments (prompt), receive PayBill payments (receive), \
                  send money to phones (send), and manage your Daraja setup."
)]
struct Cli {
    /// Emit machine-readable JSON for inspection commands.
    #[arg(long)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    // ── Collect money from a customer ───────────────────────────
    /// Send M-Pesa PIN prompt to customer's phone
    #[command(name = "prompt", visible_alias = "stk")]
    Prompt {
        /// Customer phone number (254XXXXXXXXX or 07XXXXXXXX)
        #[arg(long)]
        phone: String,
        /// Amount in KES (whole shillings; decimals are truncated)
        #[arg(long)]
        amount: String,
        /// Payment description shown on the customer's phone
        #[arg(long, default_value = "Payment")]
        description: String,
        /// Your order or invoice reference shown on the prompt
        #[arg(long, default_value = "Payment")]
        reference: String,
        /// URL that receives the payment result (required in production)
        #[arg(long)]
        callback_url: Option<String>,
        /// PayBill or Till: paybill (default) or till
        #[arg(long = "type", visible_alias = "transaction-type", value_enum, default_value_t = PromptType::Paybill)]
        payment_type: PromptType,
        /// Skip the confirmation prompt before sending
        #[arg(short = 'y', long)]
        yes: bool,
        /// Show the request that would be sent without calling Daraja.
        #[arg(long)]
        dry_run: bool,
    },
    /// Check if the customer completed a payment prompt
    #[command(name = "prompt-status", visible_aliases = ["stk-query"])]
    PromptStatus {
        /// Checkout ID from a prior prompt (e.g. ws_CO_...)
        #[arg(long)]
        checkout_id: String,
    },

    // ── Receive money (PayBill / C2B) ───────────────────────────
    /// Listen for incoming PayBill payments (registers URLs in sandbox)
    #[command(name = "receive", visible_alias = "listen")]
    Receive {
        /// Local port to receive callbacks on
        #[arg(long, default_value_t = 8181)]
        port: u16,
    },
    /// Fake an incoming PayBill payment (sandbox only)
    Simulate {
        /// Customer phone number (sandbox test MSISDN)
        #[arg(long, default_value = "254708374149")]
        phone: String,
        /// Amount in KES (whole shillings; decimals are truncated)
        #[arg(long)]
        amount: String,
        /// Bill reference number
        #[arg(long, default_value = "Test")]
        reference: String,
        /// PayBill / Till shortcode (defaults to configured shortcode)
        #[arg(long)]
        shortcode: Option<String>,
    },
    /// Register PayBill callback URLs with Daraja (advanced)
    Register {
        /// PayBill / Till shortcode (defaults to configured shortcode)
        #[arg(long)]
        shortcode: Option<String>,
        /// Response type: Completed or Cancelled
        #[arg(long, default_value = "Completed")]
        response_type: String,
        /// Confirmation URL
        #[arg(long, default_value = "https://example.com/c2b/confirm")]
        confirm_url: String,
        /// Validation URL
        #[arg(long, default_value = "https://example.com/c2b/validate")]
        validation_url: String,
    },
    /// Send a test callback to your webhook server (advanced)
    Webhook {
        /// Base URL of your server (e.g. http://localhost:8181/c2b)
        url: String,
        /// Endpoint: confirm (default) or validate
        endpoint: Option<String>,
    },

    // ── Send money to someone ───────────────────────────────────
    /// Pay a phone number — salary, refund, or disbursement
    Send {
        /// Recipient phone number
        #[arg(long)]
        to: String,
        /// Amount in KES (whole shillings; decimals are truncated)
        #[arg(long)]
        amount: String,
        /// Payment remarks shown in the transaction
        #[arg(long, default_value = "Payment")]
        remarks: String,
        /// Occasion label (optional)
        #[arg(long, default_value = "")]
        occasion: String,
        /// Payment type: business (default), salary, or promotion
        #[arg(long = "type", visible_alias = "command", value_enum, default_value_t = SendType::Business)]
        payment_type: SendType,
        /// URL that receives the payment result
        #[arg(long, default_value = DEFAULT_RESULT_URL)]
        result_url: String,
        /// URL notified if the request times out
        #[arg(long, default_value = DEFAULT_TIMEOUT_URL)]
        timeout_url: String,
        /// Skip the confirmation prompt before sending
        #[arg(short = 'y', long)]
        yes: bool,
        /// Show the request that would be sent without calling Daraja.
        #[arg(long)]
        dry_run: bool,
    },

    // ── Account operations ──────────────────────────────────────
    /// Look up a transaction by M-Pesa ID
    #[command(name = "query", visible_alias = "status")]
    Query {
        /// M-Pesa transaction ID (e.g. QEJ78H8JKR)
        #[arg(long)]
        id: String,
        /// Remarks for the query
        #[arg(long, default_value = "Status check")]
        remarks: String,
        /// Occasion label (optional)
        #[arg(long, default_value = "")]
        occasion: String,
        /// URL that receives the query result
        #[arg(long, default_value = DEFAULT_RESULT_URL)]
        result_url: String,
        /// URL notified if the request times out
        #[arg(long, default_value = DEFAULT_TIMEOUT_URL)]
        timeout_url: String,
    },
    /// Check your M-Pesa account balance
    Balance {
        /// URL that receives the balance result
        #[arg(long, default_value = DEFAULT_RESULT_URL)]
        result_url: String,
        /// URL notified if the request times out
        #[arg(long, default_value = DEFAULT_TIMEOUT_URL)]
        timeout_url: String,
    },
    /// Reverse a completed payment
    #[command(name = "refund", visible_alias = "reverse")]
    Refund {
        /// M-Pesa transaction ID to reverse
        #[arg(long)]
        id: String,
        /// Amount to refund in KES (whole shillings; decimals are truncated)
        #[arg(long)]
        amount: String,
        /// Receiver party shortcode
        #[arg(long, default_value = "600000")]
        receiver: String,
        /// Receiver identifier type (11 = shortcode)
        #[arg(long, default_value = "11")]
        receiver_type: String,
        /// Refund remarks
        #[arg(long, default_value = "Refund")]
        remarks: String,
        /// Occasion label (optional)
        #[arg(long, default_value = "")]
        occasion: String,
        /// URL that receives the refund result
        #[arg(long, default_value = DEFAULT_RESULT_URL)]
        result_url: String,
        /// URL notified if the request times out
        #[arg(long, default_value = DEFAULT_TIMEOUT_URL)]
        timeout_url: String,
        /// Show the request that would be sent without calling Daraja.
        #[arg(long)]
        dry_run: bool,
    },

    // ── Setup ───────────────────────────────────────────────────
    /// Store Daraja credentials (run once)
    Init {
        /// Environment to configure (skips the interactive prompt when set)
        #[arg(long, value_enum)]
        env: Option<InitEnvironment>,
        /// Consumer key (non-interactive init)
        #[arg(long)]
        consumer_key: Option<String>,
        /// Consumer secret (non-interactive init)
        #[arg(long)]
        consumer_secret: Option<String>,
        /// Lipa Na M-Pesa passkey (non-interactive init)
        #[arg(long)]
        passkey: Option<String>,
        /// PayBill / Till shortcode (non-interactive init)
        #[arg(long)]
        shortcode: Option<String>,
        /// Initiator name for send / query / balance / refund (optional)
        #[arg(long)]
        initiator_name: Option<String>,
        /// Initiator password for send / query / balance / refund (optional)
        #[arg(long)]
        initiator_password: Option<String>,
        /// Error if required flags are missing instead of falling back to prompts
        #[arg(long)]
        non_interactive: bool,
    },
    /// Show current configuration (never exposes secrets)
    Config,
    /// Print the SDK key for CI/CD: export DARAJA_KEY=$(pesaraja export-key)
    ExportKey,
    /// Pre-flight check before going live
    Doctor,
}

#[derive(Clone, Copy, ValueEnum)]
enum InitEnvironment {
    Sandbox,
    Production,
}

#[derive(Clone, Copy, ValueEnum)]
enum PromptType {
    Paybill,
    Till,
}

impl PromptType {
    fn to_stk_transaction_type(self) -> StkTransactionType {
        match self {
            PromptType::Paybill => StkTransactionType::PayBill,
            PromptType::Till => StkTransactionType::Till,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum SendType {
    Business,
    Salary,
    Promotion,
}

impl SendType {
    fn to_b2c_command(self) -> B2cCommand {
        match self {
            SendType::Business => B2cCommand::BusinessPayment,
            SendType::Salary => B2cCommand::SalaryPayment,
            SendType::Promotion => B2cCommand::PromotionPayment,
        }
    }
}

struct InitOptions {
    env: Option<InitEnvironment>,
    consumer_key: Option<String>,
    consumer_secret: Option<String>,
    passkey: Option<String>,
    shortcode: Option<String>,
    initiator_name: Option<String>,
    initiator_password: Option<String>,
    non_interactive: bool,
}

impl InitOptions {
    fn has_all_required(&self) -> bool {
        self.env.is_some()
            && self.consumer_key.is_some()
            && self.consumer_secret.is_some()
            && self.passkey.is_some()
            && self.shortcode.is_some()
    }
}

enum InitAction {
    Fresh,
    Update(Credentials),
    Overwrite,
    Cancelled,
}

fn parse_kes_amount(amount: &str) -> Result<u64, DarajaError> {
    let amount = amount.trim();
    if amount.is_empty() {
        return Err(DarajaError::Config("amount cannot be empty".into()));
    }

    if let Some((whole, frac)) = amount.split_once('.') {
        if whole.is_empty() || !whole.chars().all(|c| c.is_ascii_digit()) {
            return Err(DarajaError::Config(format!(
                "invalid amount '{amount}': expected a whole number of KES"
            )));
        }
        if frac.is_empty() || !frac.chars().all(|c| c.is_ascii_digit()) {
            return Err(DarajaError::Config(format!(
                "invalid amount '{amount}': expected a whole number of KES"
            )));
        }
        let whole_kes: u64 = whole
            .parse()
            .map_err(|_| DarajaError::Config(format!("invalid amount '{amount}'")))?;
        if frac.chars().any(|c| c != '0') {
            eprintln!(
                "  Warning: decimal portion dropped — M-Pesa amounts are whole KES."
            );
        }
        return Ok(whole_kes);
    }

    if !amount.chars().all(|c| c.is_ascii_digit()) {
        return Err(DarajaError::Config(format!(
            "invalid amount '{amount}': expected a whole number of KES"
        )));
    }

    amount
        .parse()
        .map_err(|_| DarajaError::Config(format!("invalid amount '{amount}'")))
}

fn sdk_key_environment(key: &str) -> Option<(&'static str, &'static str)> {
    if key.starts_with("dsk_test_") {
        Some(("sandbox", "dsk_test_"))
    } else if key.starts_with("dsk_live_") {
        Some(("production", "dsk_live_"))
    } else {
        None
    }
}

fn key_source_label() -> String {
    if std::env::var("DARAJA_KEY")
        .ok()
        .is_some_and(|k| !k.is_empty())
    {
        "DARAJA_KEY environment variable".to_string()
    } else {
        format!("auto-loaded from {}", key_path().unwrap_or_default().display())
    }
}

fn load_credentials() -> Result<Credentials, DarajaError> {
    let key = load_sdk_key()?;
    let key_bytes = config::extract_key_from_sdk_key(&key)?;
    let path = credentials_path()?;
    let encrypted = std::fs::read(&path).map_err(|e| {
        DarajaError::Config(format!("failed to read credentials: {e}"))
    })?;
    config::decrypt_credentials(&encrypted, &key_bytes)
}

fn try_load_existing_credentials() -> Option<Credentials> {
    if !credentials_path().ok()?.exists() {
        return None;
    }
    load_credentials().ok()
}

fn save_credentials(credentials: &Credentials, sdk_key: &str) -> Result<(), DarajaError> {
    let key_bytes = config::extract_key_from_sdk_key(sdk_key)?;
    let encrypted = config::encrypt_credentials(credentials, &key_bytes)?;

    let creds_path = credentials_path()?;
    if let Some(parent) = creds_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            DarajaError::Config(format!("failed to create config directory: {e}"))
        })?;
    }
    std::fs::write(&creds_path, encrypted).map_err(|e| {
        DarajaError::Config(format!(
            "failed to write credentials to {}: {e}",
            creds_path.display()
        ))
    })?;
    set_private_permissions(&creds_path)?;

    let key_file = key_path()?;
    std::fs::write(&key_file, sdk_key).map_err(|e| {
        DarajaError::Config(format!("failed to write key to {}: {e}", key_file.display()))
    })?;
    let mut perms = std::fs::metadata(&key_file)
        .map_err(|e| DarajaError::Config(format!("failed to read key file metadata: {e}")))?
        .permissions();
    perms.set_mode(0o600);
    std::fs::set_permissions(&key_file, perms).map_err(|e| {
        DarajaError::Config(format!("failed to set key file permissions: {e}"))
    })?;

    Ok(())
}

fn set_private_permissions(path: &std::path::Path) -> Result<(), DarajaError> {
    let mut perms = std::fs::metadata(path)
        .map_err(|e| DarajaError::Config(format!("failed to read file metadata: {e}")))?
        .permissions();
    perms.set_mode(0o600);
    std::fs::set_permissions(path, perms)
        .map_err(|e| DarajaError::Config(format!("failed to set private file permissions: {e}")))
}

fn personal_profile(env_label: &str, credentials: &Credentials) -> Result<Profile, DarajaError> {
    Ok(Profile {
        name: "personal".into(),
        profile_type: ProfileType::Personal,
        environment: if env_label == "sandbox" {
            Environment::Sandbox
        } else {
            Environment::Production
        },
        merchant_number: credentials.shortcode.clone(),
        credentials_path: profile_credentials_path("personal")?
            .display()
            .to_string(),
        initiator_configured: credentials.initiator_name.is_some()
            && credentials.initiator_password.is_some(),
    })
}

fn persist_personal_configuration(
    env_label: &str,
    credentials: &Credentials,
    sdk_key: &str,
) -> Result<(), DarajaError> {
    let profile = personal_profile(env_label, credentials)?;
    save_profile(&profile, credentials, sdk_key)?;
    save_credentials(credentials, sdk_key)
}

fn confirm_payment(
    phone: &str,
    amount: u64,
    description: &str,
    skip: bool,
) -> Result<(), DarajaError> {
    println!("  Phone: {}", phone);
    println!("  Amount: KES {}", amount);
    println!("  Description: {}", description);
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

fn print_dry_run(title: &str, fields: &[(&str, String)]) {
    println!("↯ Dry run: {title}");
    println!();
    for (label, value) in fields {
        println!("  {label}: {value}");
    }
    println!();
    println!("  No request was sent.");
    println!();
}

fn theme() -> ColorfulTheme {
    ColorfulTheme::default()
}

fn credentials_path() -> Result<PathBuf, DarajaError> {
    let home = std::env::var("HOME")
        .map_err(|_| DarajaError::Config("HOME environment variable is not set".into()))?;
    Ok(PathBuf::from(format!("{home}/.config/daraja/credentials.enc")))
}

fn key_path() -> Result<PathBuf, DarajaError> {
    let home = std::env::var("HOME")
        .map_err(|_| DarajaError::Config("HOME environment variable is not set".into()))?;
    Ok(PathBuf::from(format!("{home}/.config/daraja/key")))
}

/// Load the SDK key: try DARAJA_KEY env var first, then fall back to ~/.config/daraja/key.
pub(crate) fn load_sdk_key() -> Result<String, DarajaError> {
    if let Ok(key) = std::env::var("DARAJA_KEY") {
        if !key.is_empty() {
            return Ok(key);
        }
    }
    let path = key_path()?;
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

fn normalize_phone(phone: &str) -> String {
    let phone = phone.trim().trim_start_matches('+');
    if phone.starts_with('0') {
        format!("254{}", &phone[1..])
    } else {
        phone.to_string()
    }
}

fn print_error(err: &DarajaError) {
    eprintln!("× {err}");
}

fn print_banner() {
    println!();
    println!("  █▀▄ ▄▀█ █▀█ ▄▀█ ░░█ ▄▀█   █▀ █▀▄ █▄▀");
    println!("  █▄▀ █▀█ █▀▄ █▀█ █▄█ █▀█   █▄ █▀▄ █░█");
    println!();
    println!("  Setting up your M-Pesa API key.");
    println!();
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            print_error(&err);
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), DarajaError> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init {
            env,
            consumer_key,
            consumer_secret,
            passkey,
            shortcode,
            initiator_name,
            initiator_password,
            non_interactive,
        } => {
            cmd_init(InitOptions {
                env,
                consumer_key,
                consumer_secret,
                passkey,
                shortcode,
                initiator_name,
                initiator_password,
                non_interactive,
            })
            .await
        }
        Commands::Config => cmd_config(cli.json).await,
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
            )
            .await
        }
        Commands::PromptStatus { checkout_id } => cmd_prompt_status(checkout_id, cli.json).await,
        Commands::Receive { port } => receive::run(port).await,
        Commands::Simulate {
            phone,
            amount,
            reference,
            shortcode,
        } => cmd_simulate(phone, amount, reference, shortcode, cli.json).await,
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
        } => {
            cmd_send(to, amount, remarks, occasion, payment_type, result_url, timeout_url, yes, dry_run, cli.json)
                .await
        }
        Commands::Query {
            id,
            remarks,
            occasion,
            result_url,
            timeout_url,
        } => cmd_query(id, remarks, occasion, result_url, timeout_url, cli.json).await,
        Commands::Balance {
            result_url,
            timeout_url,
        } => cmd_balance(result_url, timeout_url, cli.json).await,
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
            )
            .await
        }
        Commands::Register {
            shortcode,
            response_type,
            confirm_url,
            validation_url,
        } => cmd_register(
            shortcode,
            response_type,
            confirm_url,
            validation_url,
            cli.json,
        )
        .await,
        Commands::Webhook { url, endpoint } => cmd_webhook(url, endpoint, cli.json).await,
        Commands::Doctor => cmd_doctor(cli.json).await,
    }
}

fn init_environment_label(env: InitEnvironment) -> &'static str {
    match env {
        InitEnvironment::Sandbox => "sandbox",
        InitEnvironment::Production => "production",
    }
}

fn init_key_prefix(env_label: &str) -> &'static str {
    if env_label == "sandbox" {
        "test"
    } else {
        "live"
    }
}

fn prompt_existing_credentials_action(existing: &Credentials) -> Result<InitAction, DarajaError> {
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

fn build_initiator_credentials(
    name: Option<String>,
    password: Option<String>,
) -> Result<(Option<Zeroizing<String>>, Option<Zeroizing<String>>), DarajaError> {
    match (name, password) {
        (Some(n), Some(p)) => Ok((Some(Zeroizing::new(n)), Some(Zeroizing::new(p)))),
        (None, None) => Ok((None, None)),
        _ => Err(DarajaError::Config(
            "initiator requires both --initiator-name and --initiator-password".into(),
        )),
    }
}

fn looks_like_msisdn(shortcode: &str) -> bool {
    let shortcode = shortcode.trim().trim_start_matches('+');
    shortcode.chars().all(|c| c.is_ascii_digit())
        && ((shortcode.starts_with("07") || shortcode.starts_with("01")) && shortcode.len() == 10
            || shortcode.starts_with("254") && shortcode.len() == 12)
}

fn validate_shortcode(shortcode: String) -> Result<String, DarajaError> {
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

fn validate_credentials_for_environment(
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

fn credentials_from_init_options(opts: &InitOptions) -> Result<Credentials, DarajaError> {
    let (initiator_name, initiator_password) = build_initiator_credentials(
        opts.initiator_name.clone(),
        opts.initiator_password.clone(),
    )?;

    let env = opts
        .env
        .ok_or_else(|| DarajaError::Config("missing --env".into()))?;
    let passkey = match env {
        InitEnvironment::Sandbox => SANDBOX_PASSKEY.to_string(),
        InitEnvironment::Production => opts
            .passkey
            .clone()
            .ok_or_else(|| DarajaError::Config("missing --passkey".into()))?,
    };

    Ok(Credentials {
        consumer_key: Zeroizing::new(
            opts.consumer_key
                .clone()
                .ok_or_else(|| DarajaError::Config("missing --consumer-key".into()))?,
        ),
        consumer_secret: Zeroizing::new(
            opts.consumer_secret
                .clone()
                .ok_or_else(|| DarajaError::Config("missing --consumer-secret".into()))?,
        ),
        passkey: Zeroizing::new(passkey),
        shortcode: validate_shortcode(
            opts.shortcode
                .clone()
                .ok_or_else(|| DarajaError::Config("missing --shortcode".into()))?,
        )?,
        initiator_name,
        initiator_password,
    })
}

fn prompt_credentials_interactive(
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
        defaults
            .map(|c| c.passkey.to_string())
            .unwrap_or(passkey)
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

    let had_initiator = defaults
        .and_then(|c| c.initiator_name.as_ref())
        .is_some();
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

        (
            Some(Zeroizing::new(name)),
            Some(Zeroizing::new(password)),
        )
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

fn print_init_success(env_label: &str, sdk_key: &str) -> Result<(), DarajaError> {
    let key_path = key_path()?;
    println!();
    println!("  ✓ Encrypted credentials saved.");
    println!("  ✓ SDK key saved to {} (owner-read-only)", key_path.display());
    println!();
    println!("  Your SDK key:");
    println!();
    println!("    {sdk_key}");
    println!();
    println!("  The key is auto-loaded by all pesaraja commands. No env var needed.");
    println!("  For CI/CD or Docker, set DARAJA_KEY=\"{sdk_key}\"");
    println!();
    if env_label == "sandbox" {
        println!("  Sandbox API certificate saved to ~/.config/daraja/sandbox.cer");
        println!("  Add production credentials later with:");
        println!("    pesaraja init --env production");
    } else {
        println!("  For send / query / balance / refund in production, save the");
        println!("  Safaricom API certificate from the Daraja portal to:");
        println!("    ~/.config/daraja/production.cer");
    }
    println!();
    Ok(())
}

async fn cmd_init(opts: InitOptions) -> Result<(), DarajaError> {
    if opts.non_interactive && !opts.has_all_required() {
        return Err(DarajaError::Config(
            "non-interactive init requires --env, --consumer-key, --consumer-secret, \
             --passkey, and --shortcode"
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

async fn cmd_config(json: bool) -> Result<(), DarajaError> {
    let creds_path = credentials_path()?;
    let key = load_sdk_key()?;
    let (env_name, env_prefix) = sdk_key_environment(&key).unwrap_or(("unknown", "unknown"));
    let registry = load_registry().unwrap_or_default();

    if !registry.profiles.is_empty() {
        if json {
            let output = serde_json::json!({
                "active_profile": registry.active_profile,
                "profiles": registry.profiles.iter().map(profile_summary).collect::<Vec<_>>(),
                "sdk_key_source": key_source_label(),
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
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
                .is_some_and(|name| name == profile.name);
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
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
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

pub(crate) async fn load_daraja() -> Result<Daraja, DarajaError> {
    if let Some((profile, credentials)) = load_active_profile_context()? {
        validate_credentials_for_environment(profile.environment, &credentials)?;
        validate_shortcode(credentials.shortcode.clone())?;
        return Daraja::from_credentials(credentials, profile.environment);
    }

    let key = load_sdk_key()?;
    unsafe { std::env::set_var("DARAJA_KEY", &key); }
    Daraja::from_env()
}

pub(crate) fn load_shortcode() -> Result<String, DarajaError> {
    if let Some((profile, credentials)) = load_active_profile_context()? {
        validate_credentials_for_environment(profile.environment, &credentials)?;
        return validate_shortcode(credentials.shortcode);
    }

    let key = load_sdk_key()?;
    let key_bytes = config::extract_key_from_sdk_key(&key)?;
    let path = credentials_path()?;
    let encrypted = std::fs::read(&path).map_err(|e| {
        DarajaError::Config(format!("failed to read credentials: {e}"))
    })?;
    let creds = config::decrypt_credentials(&encrypted, &key_bytes)?;
    let environment = if key.starts_with("dsk_test_") {
        Environment::Sandbox
    } else {
        Environment::Production
    };
    validate_credentials_for_environment(environment, &creds)?;
    validate_shortcode(creds.shortcode)
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
        std::fs::write(config_dir.join("key"), &sdk_key).expect("write key");
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
        persist_personal_configuration("sandbox", &first, &sdk_key)
            .expect("initial persist");

        let (active, loaded) = load_active_profile_context()
            .expect("load active context")
            .expect("active profile");
        assert_eq!(active.merchant_number, "174379");
        assert_eq!(loaded.shortcode, "174379");

        let updated = test_credentials("254743121299");
        persist_personal_configuration("sandbox", &updated, &sdk_key)
            .expect("updated persist");

        let (active, loaded) = load_active_profile_context()
            .expect("reload active context")
            .expect("active profile after update");
        assert_eq!(active.merchant_number, "254743121299");
        assert_eq!(loaded.shortcode, "254743121299");
    }

    #[test]
    fn init_options_reject_phone_number_shortcodes() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let opts = InitOptions {
            env: Some(InitEnvironment::Sandbox),
            consumer_key: Some("ck".into()),
            consumer_secret: Some("cs".into()),
            passkey: Some("pk".into()),
            shortcode: Some("0743121299".into()),
            initiator_name: None,
            initiator_password: None,
            non_interactive: true,
        };

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
            consumer_secret: Some("cs".into()),
            passkey: Some("pk".into()),
            shortcode: Some("174379".into()),
            initiator_name: None,
            initiator_password: None,
            non_interactive: true,
        };

        let creds = credentials_from_init_options(&opts).expect("business shortcode should pass");
        assert_eq!(creds.shortcode, "174379");
    }

    #[test]
    fn init_options_override_sandbox_passkey_to_documented_value() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let opts = InitOptions {
            env: Some(InitEnvironment::Sandbox),
            consumer_key: Some("ck".into()),
            consumer_secret: Some("cs".into()),
            passkey: Some("wrong-passkey".into()),
            shortcode: Some("174379".into()),
            initiator_name: None,
            initiator_password: None,
            non_interactive: true,
        };

        let creds = credentials_from_init_options(&opts)
            .expect("sandbox init should accept and override passkey");
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
}

async fn cmd_prompt(
    phone: String,
    amount: String,
    description: String,
    reference: String,
    callback_url: Option<String>,
    payment_type: PromptType,
    yes: bool,
    dry_run: bool,
) -> Result<(), DarajaError> {
    let amount = parse_kes_amount(&amount)?;
    let phone = normalize_phone(&phone);

    let callback = match callback_url {
        Some(url) => url,
        None => {
            let key = load_sdk_key()?;
            if key.starts_with("dsk_live_") {
                return Err(DarajaError::Config(
                    "callback URL is required for production — pass --callback-url".into(),
                ));
            }
            println!("  Using sandbox callback URL: {DEFAULT_CALLBACK}");
            DEFAULT_CALLBACK.to_string()
        }
    };

    if dry_run {
        print_dry_run(
            "STK Push",
            &[
                ("Phone", phone.clone()),
                ("Amount", format!("KES {amount}")),
                ("Description", description.clone()),
                ("Reference", reference.clone()),
                ("Callback URL", callback.clone()),
                (
                    "Type",
                    match payment_type {
                        PromptType::Paybill => "paybill".into(),
                        PromptType::Till => "till".into(),
                    },
                ),
            ],
        );
        return Ok(());
    }

    let daraja = load_daraja().await?;
    println!();
    confirm_payment(&phone, amount, &description, yes)?;
    println!();

    let resp = stk_push_with_type(
        daraja.client(),
        &phone,
        amount,
        &description,
        &reference,
        &callback,
        payment_type.to_stk_transaction_type(),
    )
    .await?;

    println!("✓ Payment prompt sent");
    println!();
    println!("  Merchant Request ID:  {}", resp.merchant_request_id);
    println!("  Checkout Request ID:  {}", resp.checkout_request_id);
    println!("  Response Code:        {}", resp.response_code);
    println!("  Response Description: {}", resp.response_description);
    println!("  Customer Message:     {}", resp.customer_message);
    println!();

    Ok(())
}

async fn cmd_prompt_status(checkout_id: String, json: bool) -> Result<(), DarajaError> {
    let daraja = load_daraja().await?;

    let resp = stk_push_query(daraja.client(), &checkout_id).await?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "merchant_request_id": resp.merchant_request_id,
                "checkout_request_id": resp.checkout_request_id,
                "response_code": resp.response_code,
                "response_description": resp.response_description,
                "customer_message": resp.customer_message,
            }))
            .unwrap()
        );
    } else {
        println!("✓ Prompt status retrieved");
        println!();
        println!("  Merchant Request ID:  {}", resp.merchant_request_id);
        println!("  Checkout Request ID:  {}", resp.checkout_request_id);
        println!("  Response Code:        {}", resp.response_code);
        println!("  Response Description: {}", resp.response_description);
        println!("  Customer Message:     {}", resp.customer_message);
        println!();
    }

    Ok(())
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
    let phone = normalize_phone(&phone);

    let resp = simulate_c2b(
        daraja.client(),
        &shortcode,
        amount,
        &phone,
        &reference,
    )
    .await?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "conversation_id": resp.conversation_id,
                "originator_conversation_id": resp.originator_conversation_id,
                "response_description": resp.response_description,
            }))
            .unwrap()
        );
    } else {
        println!("✓ C2B payment simulated");
        println!();
        println!("  Conversation ID:           {}", resp.conversation_id);
        println!("  Originator Conversation ID: {}", resp.originator_conversation_id);
        println!("  Response Description:      {}", resp.response_description);
        println!();
        println!("  Run `pesaraja receive` to catch the confirmation callback.");
        println!();
    }

    Ok(())
}

async fn cmd_send(
    to: String,
    amount: String,
    remarks: String,
    occasion: String,
    payment_type: SendType,
    result_url: String,
    timeout_url: String,
    yes: bool,
    dry_run: bool,
    json: bool,
) -> Result<(), DarajaError> {
    let amount = parse_kes_amount(&amount)?;
    let phone = normalize_phone(&to);

    if dry_run {
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
            ],
        );
        return Ok(());
    }

    let daraja = load_daraja().await?;
    println!();
    confirm_payment(&phone, amount, &remarks, yes)?;
    println!();

    let resp = b2c_payment(
        daraja.client(),
        &phone,
        amount,
        payment_type.to_b2c_command(),
        &remarks,
        &occasion,
        &result_url,
        &timeout_url,
    )
    .await?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "conversation_id": resp.conversation_id,
                "originator_conversation_id": resp.originator_conversation_id,
                "response_code": resp.response_code,
                "response_description": resp.response_description,
            }))
            .unwrap()
        );
    } else {
        println!("✓ Payment sent");
        println!();
        println!("  Conversation ID:           {}", resp.conversation_id);
        println!("  Originator Conversation ID: {}", resp.originator_conversation_id);
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
    result_url: String,
    timeout_url: String,
    json: bool,
) -> Result<(), DarajaError> {
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
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "conversation_id": resp.conversation_id,
                "originator_conversation_id": resp.originator_conversation_id,
                "response_code": resp.response_code,
                "response_description": resp.response_description,
            }))
            .unwrap()
        );
    } else {
        println!("✓ Transaction query sent");
        println!();
        println!("  Conversation ID:           {}", resp.conversation_id);
        println!("  Originator Conversation ID: {}", resp.originator_conversation_id);
        println!("  Response Code:             {}", resp.response_code);
        println!("  Response Description:      {}", resp.response_description);
        println!();
    }

    Ok(())
}

async fn cmd_balance(result_url: String, timeout_url: String, json: bool) -> Result<(), DarajaError> {
    let daraja = load_daraja().await?;

    let resp = query_balance(daraja.client(), &result_url, &timeout_url).await?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "conversation_id": resp.conversation_id,
                "originator_conversation_id": resp.originator_conversation_id,
                "response_code": resp.response_code,
                "response_description": resp.response_description,
                "note": "Balance arrives asynchronously at your result URL",
            }))
            .unwrap()
        );
    } else {
        println!("✓ Balance query sent");
        println!();
        println!("  Conversation ID:           {}", resp.conversation_id);
        println!("  Originator Conversation ID: {}", resp.originator_conversation_id);
        println!("  Response Code:             {}", resp.response_code);
        println!("  Response Description:      {}", resp.response_description);
        println!("  (Balance arrives asynchronously at your result URL)");
        println!();
    }

    Ok(())
}

async fn cmd_refund(
    id: String,
    amount: u64,
    receiver: String,
    receiver_type: String,
    remarks: String,
    occasion: String,
    result_url: String,
    timeout_url: String,
    dry_run: bool,
) -> Result<(), DarajaError> {
    if dry_run {
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
            ],
        );
        return Ok(());
    }

    let daraja = load_daraja().await?;

    let resp = reverse_transaction(
        daraja.client(),
        &id,
        amount,
        &receiver,
        &receiver_type,
        &remarks,
        &occasion,
        &result_url,
        &timeout_url,
    )
    .await?;

    println!("✓ Refund initiated");
    println!();
    println!("  Conversation ID:           {}", resp.conversation_id);
    println!("  Originator Conversation ID: {}", resp.originator_conversation_id);
    println!("  Response Code:             {}", resp.response_code);
    println!("  Response Description:      {}", resp.response_description);
    println!();

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
    let payload = serde_json::to_string(&body).map_err(|e| {
        DarajaError::Config(format!("failed to serialize sample payload: {e}"))
    })?;

    let resp = reqwest::Client::new()
        .post(&target)
        .header("Content-Type", "application/json")
        .header("X-Safaricom-Signature", "test-signature")
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
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "endpoint": target,
                "status": status.as_u16(),
                "body": response_body,
            }))
            .unwrap()
        );
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
    )
    .await?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "conversation_id": resp.conversation_id,
                "originator_conversation_id": resp.originator_conversation_id,
                "response_description": resp.response_description,
            }))
            .unwrap()
        );
    } else {
        println!("✓ C2B URLs registered");
        println!();
        println!("  Conversation ID:           {}", resp.conversation_id);
        println!("  Originator Conversation ID: {}", resp.originator_conversation_id);
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
            format!(
                "Credentials encrypted at rest ({})",
                creds_path.display()
            )
        } else {
            "Credentials file not found".to_string()
        },
        hint: if creds_exist {
            None
        } else {
            Some("Run `pesaraja init` to set up.".to_string())
        },
    });

    let daraja_key = load_sdk_key().ok();
    let key_set = daraja_key.is_some();
    let key_from_env = std::env::var("DARAJA_KEY").ok().is_some();

    checks.push(CheckItem {
        ok: key_set,
        message: if key_set {
            if key_from_env {
                "DARAJA_KEY is set (env var)".to_string()
            } else {
                "SDK key loaded from ~/.config/daraja/key".to_string()
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

        let sandbox_cert_ok = std::env::var("DARAJA_CERT_PATH")
            .map(|p| std::path::Path::new(&p).exists())
            .or_else(|_| {
                std::env::var("DARAJA_SANDBOX_CERT_PATH")
                    .map(|p| std::path::Path::new(&p).exists())
            })
            .unwrap_or_else(|_| {
                std::env::var("HOME")
                    .map(|home| {
                        std::path::Path::new(&format!("{home}/.config/daraja/sandbox.cer"))
                            .exists()
                    })
                    .unwrap_or(false)
            });
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
                Some("Run `pesaraja init` to download ~/.config/daraja/sandbox.cer.".into())
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

        let cert_ok = std::env::var("DARAJA_CERT_PATH")
            .map(|p| std::path::Path::new(&p).exists())
            .unwrap_or_else(|_| {
                std::env::var("HOME")
                    .map(|home| {
                        std::path::Path::new(&format!("{home}/.config/daraja/production.cer"))
                            .exists()
                    })
                    .unwrap_or(false)
            });
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
                Some(
                    "Download from the Daraja portal and save to \
                     ~/.config/daraja/production.cer (or set DARAJA_CERT_PATH)."
                        .into(),
                )
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
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
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
