mod args;
mod cmd_c2b;
mod cmd_collect;
mod cmd_doctor;
mod cmd_init;
mod cmd_money;
mod common;
mod creds;
mod input;
#[cfg(test)]
mod legacy_tests;
mod money_mappings;
mod output;
mod payments;
mod prelude;
mod profiles;
mod receive;
mod simulator;

// Re-export the shared CLI surface so sibling modules (`receive`, `payments`)
// keep resolving `crate::{...}` paths without per-module import churn.
pub(crate) use prelude::*;

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
            cmd_init::cmd_setup(
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
        Commands::Use { name } => cmd_init::cmd_use(name),
        Commands::Init {
            env,
            consumer_key,
            shortcode,
            initiator_name,
            non_interactive,
        } => {
            cmd_init::cmd_init(InitOptions {
                env,
                consumer_key,
                shortcode,
                initiator_name,
                non_interactive,
            })
            .await
        }
        Commands::Config => cmd_init::cmd_config(json).await,
        Commands::ExportKey => cmd_init::cmd_export_key().await,
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
            cmd_collect::cmd_prompt(
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
        } => cmd_collect::cmd_prompt_status(checkout_id, base_url, json).await,
        Commands::Payments { command } => payments::run(command, json).await,
        Commands::Receive { port } => receive::run(port).await,
        Commands::Simulate {
            phone,
            amount,
            reference,
            shortcode,
        } => cmd_money::cmd_simulate(phone, amount, reference, shortcode, json).await,
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
            cmd_money::cmd_send(
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
        } => cmd_money::cmd_query(id, remarks, occasion, result_url, timeout_url, json).await,
        Commands::Balance {
            result_url,
            timeout_url,
        } => cmd_money::cmd_balance(result_url, timeout_url, json).await,
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
            cmd_money::cmd_refund(
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
        } => {
            cmd_c2b::cmd_register(shortcode, response_type, confirm_url, validation_url, json).await
        }
        Commands::Webhook { url, endpoint } => cmd_c2b::cmd_webhook(url, endpoint, json).await,
        Commands::Simulator { port, scenario } => simulator::run(port, scenario).await,
        Commands::Doctor => cmd_doctor::cmd_doctor(json).await,
    }
}
