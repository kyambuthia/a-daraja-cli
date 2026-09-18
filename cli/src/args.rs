use crate::prelude::*;

#[derive(Parser)]
#[command(
    name = "pesaraja",
    version,
    about = "M-Pesa payments — prompt, receive, and send money",
    long_about = "Collect payments (prompt), receive PayBill payments (receive), \
                  send money to phones (send), and manage your Daraja setup."
)]
pub(crate) struct Cli {
    /// Emit stable machine-readable JSON output and errors.
    #[arg(long)]
    pub(crate) json: bool,

    /// Use a named profile for this invocation instead of the active profile.
    #[arg(long, global = true)]
    pub(crate) profile: Option<String>,

    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Configure a named merchant profile without replacing personal credentials.
    Setup {
        /// Profile type: paybill, till, or pochi.
        #[arg(value_enum)]
        profile_type: SetupProfileType,
        /// Profile name (prompted when omitted).
        #[arg(long)]
        name: Option<String>,
        /// Environment (defaults to the current SDK key environment).
        #[arg(long, value_enum)]
        env: Option<InitEnvironment>,
        /// Consumer key for non-interactive setup.
        #[arg(long)]
        consumer_key: Option<String>,
        /// Merchant shortcode for non-interactive setup.
        #[arg(long)]
        shortcode: Option<String>,
        /// Initiator name for send / query / balance / refund.
        #[arg(long)]
        initiator_name: Option<String>,
        /// Error if required flags and secret environment variables are missing.
        #[arg(long)]
        non_interactive: bool,
    },
    /// Make a saved profile active.
    Use {
        /// Profile name to activate.
        name: String,
    },
    // ── Collect money from a customer ───────────────────────────
    /// Send M-Pesa PIN prompt to customer's phone
    #[command(name = "prompt", visible_alias = "stk")]
    Prompt {
        /// Customer phone number (254XXXXXXXXX or 07XXXXXXXX)
        #[arg(long)]
        phone: String,
        /// Amount in KES (positive whole shillings)
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
        /// Stable business-operation key for safely repeating this exact request
        #[arg(long)]
        idempotency_key: Option<String>,
        /// Loopback-only Daraja endpoint for local simulator testing.
        #[arg(long)]
        base_url: Option<String>,
    },
    /// Check if the customer completed a payment prompt
    #[command(name = "prompt-status", visible_aliases = ["stk-query"])]
    PromptStatus {
        /// Checkout ID from a prior prompt (e.g. ws_CO_...)
        #[arg(long)]
        checkout_id: String,
        /// Loopback-only Daraja endpoint for local simulator testing.
        #[arg(long)]
        base_url: Option<String>,
    },
    /// Inspect and reconcile locally persisted payment records.
    Payments {
        #[command(subcommand)]
        command: PaymentCommands,
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
        /// Amount in KES (positive whole shillings)
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
    /// Run a local, deterministic Daraja-compatible HTTP simulator.
    Simulator {
        /// Local port for the simulator.
        #[arg(long, default_value_t = 8787)]
        port: u16,
        /// Response behavior to exercise.
        #[arg(long, value_enum, default_value_t = simulator::Scenario::Success)]
        scenario: simulator::Scenario,
    },

    // ── Send money to someone ───────────────────────────────────
    /// Pay a phone number — salary, refund, or disbursement
    Send {
        /// Recipient phone number
        #[arg(long)]
        to: String,
        /// Amount in KES (positive whole shillings)
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
        #[arg(long)]
        result_url: Option<String>,
        /// URL notified if the request times out
        #[arg(long)]
        timeout_url: Option<String>,
        /// Skip the confirmation prompt before sending
        #[arg(short = 'y', long)]
        yes: bool,
        /// Show the request that would be sent without calling Daraja.
        #[arg(long)]
        dry_run: bool,
        /// Stable business-operation key for safely repeating this exact request
        #[arg(long)]
        idempotency_key: Option<String>,
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
        #[arg(long)]
        result_url: Option<String>,
        /// URL notified if the request times out
        #[arg(long)]
        timeout_url: Option<String>,
    },
    /// Check your M-Pesa account balance
    Balance {
        /// URL that receives the balance result
        #[arg(long)]
        result_url: Option<String>,
        /// URL notified if the request times out
        #[arg(long)]
        timeout_url: Option<String>,
    },
    /// Reverse a completed payment
    #[command(name = "refund", visible_alias = "reverse")]
    Refund {
        /// M-Pesa transaction ID to reverse
        #[arg(long)]
        id: String,
        /// Amount to refund in KES (positive whole shillings)
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
        #[arg(long)]
        result_url: Option<String>,
        /// URL notified if the request times out
        #[arg(long)]
        timeout_url: Option<String>,
        /// Show the request that would be sent without calling Daraja.
        #[arg(long)]
        dry_run: bool,
        /// Skip the confirmation prompt before initiating the reversal.
        #[arg(short = 'y', long)]
        yes: bool,
        /// Stable business-operation key for safely repeating this exact request
        #[arg(long)]
        idempotency_key: Option<String>,
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
        /// PayBill / Till shortcode (non-interactive init)
        #[arg(long)]
        shortcode: Option<String>,
        /// Initiator name for send / query / balance / refund (optional)
        #[arg(long)]
        initiator_name: Option<String>,
        /// Error if required flags and secret environment variables are missing
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

#[derive(Subcommand)]
pub(crate) enum PaymentCommands {
    /// List persisted payments.
    List {
        /// Optional lifecycle status filter.
        #[arg(long)]
        status: Option<String>,
    },
    /// Show one payment and its immutable event history.
    Inspect {
        /// Local payment identifier.
        #[arg(long)]
        id: String,
    },
    /// Query Daraja and apply the authoritative STK result.
    Reconcile {
        /// Local payment identifier.
        #[arg(long)]
        id: String,
        /// Loopback-only Daraja endpoint for local simulator testing.
        #[arg(long)]
        base_url: Option<String>,
    },
    /// Show a safe retry plan; submission requires a future explicit workflow.
    Retry {
        /// Local payment identifier.
        #[arg(long)]
        id: String,
        /// Required to print the retry plan instead of refusing the operation.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum InitEnvironment {
    Sandbox,
    Production,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum SetupProfileType {
    Paybill,
    Till,
    Pochi,
}

impl SetupProfileType {
    pub(crate) fn to_profile_type(self) -> ProfileType {
        match self {
            Self::Paybill => ProfileType::Paybill,
            Self::Till => ProfileType::Till,
            Self::Pochi => ProfileType::Pochi,
        }
    }

    pub(crate) fn account_label(self) -> &'static str {
        match self {
            Self::Paybill => "PayBill shortcode",
            Self::Till => "Till number",
            Self::Pochi => "Pochi merchant number",
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum PromptType {
    Paybill,
    Till,
}

impl PromptType {
    pub(crate) fn to_stk_transaction_type(self) -> StkTransactionType {
        match self {
            PromptType::Paybill => StkTransactionType::PayBill,
            PromptType::Till => StkTransactionType::Till,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum SendType {
    Business,
    Salary,
    Promotion,
}

impl SendType {
    pub(crate) fn to_b2c_command(self) -> B2cCommand {
        match self {
            SendType::Business => B2cCommand::BusinessPayment,
            SendType::Salary => B2cCommand::SalaryPayment,
            SendType::Promotion => B2cCommand::PromotionPayment,
        }
    }
}

pub(crate) struct InitOptions {
    pub(crate) env: Option<InitEnvironment>,
    pub(crate) consumer_key: Option<String>,
    pub(crate) shortcode: Option<String>,
    pub(crate) initiator_name: Option<String>,
    pub(crate) non_interactive: bool,
}

impl InitOptions {
    pub(crate) fn has_all_required(&self) -> bool {
        let secrets_available = std::env::var("DARAJA_CONSUMER_SECRET")
            .ok()
            .is_some_and(|value| !value.is_empty())
            && match self.env {
                Some(InitEnvironment::Sandbox) => true,
                Some(InitEnvironment::Production) => std::env::var("DARAJA_PASSKEY")
                    .ok()
                    .is_some_and(|value| !value.is_empty()),
                None => false,
            };
        self.env.is_some()
            && self.consumer_key.is_some()
            && self.shortcode.is_some()
            && secrets_available
    }
}

pub(crate) enum InitAction {
    Fresh,
    Update(Credentials),
    Overwrite,
    Cancelled,
}
