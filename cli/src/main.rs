use clap::{Parser, Subcommand, ValueEnum};
use daraja_rs::api::balance::query_balance;
use daraja_rs::api::b2c::b2c_payment;
use daraja_rs::api::c2b::register_urls;
use daraja_rs::api::reversal::reverse_transaction;
use daraja_rs::api::status::query_status;
use daraja_rs::api::stk_push::stk_push;
use daraja_rs::auth::TokenManager;
use daraja_rs::config::{self, Credentials};
use daraja_rs::error::DarajaError;
use daraja_rs::types::{B2cCommand, Environment};
use daraja_rs::Daraja;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Password, Select};
use std::path::PathBuf;
use std::process::ExitCode;
use zeroize::Zeroizing;

const DEFAULT_CALLBACK: &str = "http://localhost:8181/callback";
const DEFAULT_RESULT_URL: &str = "http://localhost:8181/result";
const DEFAULT_TIMEOUT_URL: &str = "http://localhost:8181/timeout";

#[derive(Parser)]
#[command(
    name = "mpesa",
    version,
    about = "M-Pesa payments via Safaricom Daraja — zero ceremony",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Interactive setup wizard — encrypt and store Daraja credentials
    Init {
        /// Environment to configure (skips the interactive prompt when set)
        #[arg(long, value_enum)]
        env: Option<InitEnvironment>,
    },
    /// STK Push — Lipa Na M-Pesa Online
    Stk {
        /// Customer phone number (254XXXXXXXXX or 07XXXXXXXX)
        #[arg(long)]
        phone: String,
        /// Amount in KES
        #[arg(long)]
        amount: u64,
        /// Payment description shown to the customer
        #[arg(long, default_value = "Payment")]
        description: String,
        /// Account reference shown on the STK prompt
        #[arg(long, default_value = "Payment")]
        reference: String,
        /// Callback URL for STK result (required for production)
        #[arg(long)]
        callback_url: Option<String>,
    },
    /// B2C payment — send money to a phone number
    Send {
        /// Recipient phone number
        #[arg(long)]
        to: String,
        /// Amount in KES
        #[arg(long)]
        amount: u64,
        /// Payment remarks
        #[arg(long, default_value = "Payment")]
        remarks: String,
        /// Occasion label
        #[arg(long, default_value = "")]
        occasion: String,
        /// B2C command type
        #[arg(long, value_enum, default_value_t = SendCommand::Business)]
        command: SendCommand,
        /// Result callback URL
        #[arg(long, default_value = DEFAULT_RESULT_URL)]
        result_url: String,
        /// Timeout callback URL
        #[arg(long, default_value = DEFAULT_TIMEOUT_URL)]
        timeout_url: String,
    },
    /// Query transaction status
    Status {
        /// M-Pesa transaction ID
        #[arg(long)]
        id: String,
        /// Remarks for the query
        #[arg(long, default_value = "Status check")]
        remarks: String,
        /// Occasion label
        #[arg(long, default_value = "")]
        occasion: String,
        /// Result callback URL
        #[arg(long, default_value = DEFAULT_RESULT_URL)]
        result_url: String,
        /// Timeout callback URL
        #[arg(long, default_value = DEFAULT_TIMEOUT_URL)]
        timeout_url: String,
    },
    /// Query M-Pesa account balance
    Balance {
        /// Result callback URL
        #[arg(long, default_value = DEFAULT_RESULT_URL)]
        result_url: String,
        /// Timeout callback URL
        #[arg(long, default_value = DEFAULT_TIMEOUT_URL)]
        timeout_url: String,
    },
    /// Reverse a completed transaction
    Reverse {
        /// M-Pesa transaction ID to reverse
        #[arg(long)]
        id: String,
        /// Amount to reverse in KES
        #[arg(long)]
        amount: u64,
        /// Receiver party shortcode
        #[arg(long, default_value = "600000")]
        receiver: String,
        /// Receiver identifier type (11 = shortcode)
        #[arg(long, default_value = "11")]
        receiver_type: String,
        /// Reversal remarks
        #[arg(long, default_value = "Reversal")]
        remarks: String,
        /// Occasion label
        #[arg(long, default_value = "")]
        occasion: String,
        /// Result callback URL
        #[arg(long, default_value = DEFAULT_RESULT_URL)]
        result_url: String,
        /// Timeout callback URL
        #[arg(long, default_value = DEFAULT_TIMEOUT_URL)]
        timeout_url: String,
    },
    /// Register C2B validation and confirmation URLs
    Register {
        /// PayBill / Till shortcode (defaults to configured shortcode)
        #[arg(long)]
        shortcode: Option<String>,
        /// Response type: Completed or Cancelled
        #[arg(long, default_value = "Completed")]
        response_type: String,
        /// Confirmation URL
        #[arg(long, default_value = "http://localhost:8181/c2b/confirm")]
        confirmation_url: String,
        /// Validation URL
        #[arg(long, default_value = "http://localhost:8181/c2b/validate")]
        validation_url: String,
    },
    /// Production readiness check
    Doctor,
}

#[derive(Clone, Copy, ValueEnum)]
enum InitEnvironment {
    Sandbox,
    Production,
}

#[derive(Clone, Copy, ValueEnum)]
enum SendCommand {
    Business,
    Salary,
    Promotion,
}

impl SendCommand {
    fn to_b2c_command(self) -> B2cCommand {
        match self {
            SendCommand::Business => B2cCommand::BusinessPayment,
            SendCommand::Salary => B2cCommand::SalaryPayment,
            SendCommand::Promotion => B2cCommand::PromotionPayment,
        }
    }
}

fn theme() -> ColorfulTheme {
    ColorfulTheme::default()
}

fn credentials_path() -> Result<PathBuf, DarajaError> {
    let home = std::env::var("HOME")
        .map_err(|_| DarajaError::Config("HOME environment variable is not set".into()))?;
    Ok(PathBuf::from(format!("{home}/.config/daraja/credentials.enc")))
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
        Commands::Init { env } => cmd_init(env).await,
        Commands::Stk {
            phone,
            amount,
            description,
            reference,
            callback_url,
        } => cmd_stk(phone, amount, description, reference, callback_url).await,
        Commands::Send {
            to,
            amount,
            remarks,
            occasion,
            command,
            result_url,
            timeout_url,
        } => {
            cmd_send(to, amount, remarks, occasion, command, result_url, timeout_url).await
        }
        Commands::Status {
            id,
            remarks,
            occasion,
            result_url,
            timeout_url,
        } => cmd_status(id, remarks, occasion, result_url, timeout_url).await,
        Commands::Balance {
            result_url,
            timeout_url,
        } => cmd_balance(result_url, timeout_url).await,
        Commands::Reverse {
            id,
            amount,
            receiver,
            receiver_type,
            remarks,
            occasion,
            result_url,
            timeout_url,
        } => {
            cmd_reverse(
                id,
                amount,
                receiver,
                receiver_type,
                remarks,
                occasion,
                result_url,
                timeout_url,
            )
            .await
        }
        Commands::Register {
            shortcode,
            response_type,
            confirmation_url,
            validation_url,
        } => {
            cmd_register(
                shortcode,
                response_type,
                confirmation_url,
                validation_url,
            )
            .await
        }
        Commands::Doctor => cmd_doctor().await,
    }
}

async fn cmd_init(env_flag: Option<InitEnvironment>) -> Result<(), DarajaError> {
    print_banner();

    let env_label = match env_flag {
        Some(InitEnvironment::Sandbox) => "sandbox",
        Some(InitEnvironment::Production) => "production",
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

    let key_prefix = if env_label == "sandbox" { "test" } else { "live" };

    let consumer_key: String = Input::with_theme(&theme())
        .with_prompt("Consumer Key")
        .interact_text()
        .map_err(|e| DarajaError::Config(format!("prompt cancelled: {e}")))?;

    let consumer_secret: String = Password::with_theme(&theme())
        .with_prompt("Consumer Secret")
        .interact()
        .map_err(|e| DarajaError::Config(format!("prompt cancelled: {e}")))?;

    let passkey: String = Password::with_theme(&theme())
        .with_prompt("Passkey")
        .interact()
        .map_err(|e| DarajaError::Config(format!("prompt cancelled: {e}")))?;

    let default_shortcode = if env_label == "sandbox" {
        "174379"
    } else {
        ""
    };
    let shortcode: String = Input::with_theme(&theme())
        .with_prompt("Shortcode")
        .default(default_shortcode.to_string())
        .interact_text()
        .map_err(|e| DarajaError::Config(format!("prompt cancelled: {e}")))?;

    println!();
    println!("  For B2C / transaction status / reversal (optional):");
    println!();

    let add_initiator = Confirm::with_theme(&theme())
        .with_prompt("Configure initiator credentials?")
        .default(env_label == "sandbox")
        .interact()
        .map_err(|e| DarajaError::Config(format!("prompt cancelled: {e}")))?;

    let (initiator_name, initiator_password) = if add_initiator {
        let name: String = Input::with_theme(&theme())
            .with_prompt("Initiator Name")
            .default(if env_label == "sandbox" {
                "apitest".to_string()
            } else {
                String::new()
            })
            .interact_text()
            .map_err(|e| DarajaError::Config(format!("prompt cancelled: {e}")))?;

        let password: String = Password::with_theme(&theme())
            .with_prompt("Initiator Password")
            .interact()
            .map_err(|e| DarajaError::Config(format!("prompt cancelled: {e}")))?;

        (
            Some(Zeroizing::new(name)),
            Some(Zeroizing::new(password)),
        )
    } else {
        (None, None)
    };

    let credentials = Credentials {
        consumer_key: Zeroizing::new(consumer_key),
        consumer_secret: Zeroizing::new(consumer_secret),
        passkey: Zeroizing::new(passkey),
        shortcode,
        initiator_name,
        initiator_password,
    };

    let sdk_key = config::generate_sdk_key(key_prefix);
    let key_bytes = config::extract_key_from_sdk_key(&sdk_key)?;
    let encrypted = config::encrypt_credentials(&credentials, &key_bytes)?;

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

    println!();
    println!("  ✓ Encrypted credentials saved.");
    println!();
    println!("  Your SDK key:");
    println!();
    println!("    {sdk_key}   ← THIS IS THE ONLY THING YOU NEED TO SAVE");
    println!();
    println!("  Add to your .env:");
    println!("    DARAJA_KEY=\"{sdk_key}\"");
    println!();
    if env_label == "sandbox" {
        println!("  Add production credentials later with:");
        println!("    mpesa init --env production");
    }
    println!();

    Ok(())
}

async fn load_daraja() -> Result<Daraja, DarajaError> {
    Daraja::from_env()
}

fn load_shortcode() -> Result<String, DarajaError> {
    let key = std::env::var("DARAJA_KEY").map_err(|_| DarajaError::MissingApiKey)?;
    let key_bytes = config::extract_key_from_sdk_key(&key)?;
    let path = credentials_path()?;
    let encrypted = std::fs::read(&path).map_err(|e| {
        DarajaError::Config(format!("failed to read credentials: {e}"))
    })?;
    let creds = config::decrypt_credentials(&encrypted, &key_bytes)?;
    Ok(creds.shortcode)
}

async fn cmd_stk(
    phone: String,
    amount: u64,
    description: String,
    reference: String,
    callback_url: Option<String>,
) -> Result<(), DarajaError> {
    let daraja = load_daraja().await?;
    let phone = normalize_phone(&phone);

    let callback = match callback_url {
        Some(url) => url,
        None => {
            let key = std::env::var("DARAJA_KEY").map_err(|_| DarajaError::MissingApiKey)?;
            if key.starts_with("dsk_live_") {
                return Err(DarajaError::Config(
                    "callback URL is required for production — pass --callback-url".into(),
                ));
            }
            println!("  Using local callback URL: {DEFAULT_CALLBACK}");
            println!("  Run `mpesa listen` or expose via ngrok for real callbacks.");
            println!();
            DEFAULT_CALLBACK.to_string()
        }
    };

    let resp = stk_push(
        daraja.client(),
        &phone,
        amount,
        &description,
        &reference,
        &callback,
    )
    .await?;

    println!("✓ STK Push initiated");
    println!();
    println!("  Merchant Request ID:  {}", resp.merchant_request_id);
    println!("  Checkout Request ID:  {}", resp.checkout_request_id);
    println!("  Response Code:        {}", resp.response_code);
    println!("  Response Description: {}", resp.response_description);
    println!("  Customer Message:     {}", resp.customer_message);
    println!();

    Ok(())
}

async fn cmd_send(
    to: String,
    amount: u64,
    remarks: String,
    occasion: String,
    command: SendCommand,
    result_url: String,
    timeout_url: String,
) -> Result<(), DarajaError> {
    let daraja = load_daraja().await?;
    let phone = normalize_phone(&to);

    let resp = b2c_payment(
        daraja.client(),
        &phone,
        amount,
        command.to_b2c_command(),
        &remarks,
        &occasion,
        &result_url,
        &timeout_url,
    )
    .await?;

    println!("✓ B2C payment initiated");
    println!();
    println!("  Conversation ID:           {}", resp.conversation_id);
    println!("  Originator Conversation ID: {}", resp.originator_conversation_id);
    println!("  Response Code:             {}", resp.response_code);
    println!("  Response Description:      {}", resp.response_description);
    println!();

    Ok(())
}

async fn cmd_status(
    id: String,
    remarks: String,
    occasion: String,
    result_url: String,
    timeout_url: String,
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

    println!("✓ Transaction status query sent");
    println!();
    println!("  Conversation ID:           {}", resp.conversation_id);
    println!("  Originator Conversation ID: {}", resp.originator_conversation_id);
    println!("  Response Code:             {}", resp.response_code);
    println!("  Response Description:      {}", resp.response_description);
    println!();

    Ok(())
}

async fn cmd_balance(result_url: String, timeout_url: String) -> Result<(), DarajaError> {
    let daraja = load_daraja().await?;

    let resp = query_balance(daraja.client(), &result_url, &timeout_url).await?;

    println!("✓ Balance query sent");
    println!();
    println!("  Conversation ID:           {}", resp.conversation_id);
    println!("  Originator Conversation ID: {}", resp.originator_conversation_id);
    println!("  Response Code:             {}", resp.response_code);
    println!("  Response Description:      {}", resp.response_description);
    println!("  (Balance arrives asynchronously at your result URL)");
    println!();

    Ok(())
}

async fn cmd_reverse(
    id: String,
    amount: u64,
    receiver: String,
    receiver_type: String,
    remarks: String,
    occasion: String,
    result_url: String,
    timeout_url: String,
) -> Result<(), DarajaError> {
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

    println!("✓ Reversal initiated");
    println!();
    println!("  Conversation ID:           {}", resp.conversation_id);
    println!("  Originator Conversation ID: {}", resp.originator_conversation_id);
    println!("  Response Code:             {}", resp.response_code);
    println!("  Response Description:      {}", resp.response_description);
    println!();

    Ok(())
}

async fn cmd_register(
    shortcode: Option<String>,
    response_type: String,
    confirmation_url: String,
    validation_url: String,
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
        &confirmation_url,
        &validation_url,
    )
    .await?;

    println!("✓ C2B URLs registered");
    println!();
    println!("  Conversation ID:           {}", resp.conversation_id);
    println!("  Originator Conversation ID: {}", resp.originator_conversation_id);
    println!("  Response Description:      {}", resp.response_description);
    println!();

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

async fn cmd_doctor() -> Result<(), DarajaError> {
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
            Some("Run `mpesa init` to set up.".to_string())
        },
    });

    let daraja_key = std::env::var("DARAJA_KEY").ok();
    let key_set = daraja_key.is_some();
    checks.push(CheckItem {
        ok: key_set,
        message: if key_set {
            "DARAJA_KEY is set".to_string()
        } else {
            "DARAJA_KEY is not set".to_string()
        },
        hint: if key_set {
            None
        } else {
            Some("Export the key from `mpesa init` into your .env.".to_string())
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
                hint: Some("Re-run `mpesa init` to generate a valid key.".into()),
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
                                            "Check your consumer key and secret in `mpesa init`."
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
                                        "Required for B2C, status, balance, and reversal. \
                                         Re-run `mpesa init` and add initiator details."
                                            .into(),
                                    ),
                                });
                            }

                            if is_sandbox {
                                checks.push(CheckItem {
                                    ok: false,
                                    message: "B2C: not available in sandbox (needs Go Live)"
                                        .into(),
                                    hint: None,
                                });
                            } else if has_initiator {
                                checks.push(CheckItem {
                                    ok: true,
                                    message: "B2C: credentials ready".into(),
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
        checks.push(CheckItem {
            ok: false,
            message: "Production credentials not configured".into(),
            hint: Some("Run `mpesa init --env production` to set up.".into()),
        });
    } else if is_production {
        checks.push(CheckItem {
            ok: true,
            message: "Production environment configured (dsk_live_)".into(),
            hint: None,
        });
    }

    checks.push(CheckItem {
        ok: true,
        message: format!("Default callback URL: {DEFAULT_CALLBACK} (local dev)"),
        hint: Some(
            "For production, expose a public HTTPS endpoint (e.g. ngrok) and pass --callback-url."
                .into(),
        ),
    });

    println!();
    for check in &checks {
        print_check(check);
    }
    println!();

    let critical_failed = !creds_exist
        || !key_set
        || checks.iter().any(|c| {
            c.message.contains("OAuth failed")
                || c.message.contains("Failed to decrypt")
                || c.message.contains("invalid prefix")
                || c.message.contains("Invalid SDK key")
        });

    if critical_failed {
        return Err(DarajaError::Config(
            "critical checks failed — review the report above".into(),
        ));
    }

    Ok(())
}