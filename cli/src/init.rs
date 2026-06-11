use anyhow::{Context, Result};
use console::{style, Term};
use dialoguer::{Confirm, Input, Password, Select};
use pesaraja::config;
use pesaraja::{Credentials, Environment};

pub async fn run() -> Result<()> {
    let term = Term::stdout();
    term.clear_screen().ok();

    term.write_line("").ok();
    term.write_line(
        &style("  █▀▄  ▄▀█  █▀█  ▄▀█  ░░█  ▄▀█    █▀  █▀▄  █▄▀")
            .cyan()
            .bold()
            .to_string(),
    )
    .ok();
    term.write_line(
        &style("  █▄▀  █▀█  █▀▄  █▀█  █▄█  █▀█    █▄  █▀▄  █░█")
            .cyan()
            .bold()
            .to_string(),
    )
    .ok();
    term.write_line("").ok();
    term.write_line(&style("  Setting up your M-Pesa API key").dim().to_string()).ok();
    term.write_line("").ok();

    let envs = &["Sandbox (testing)", "Production (live)"];
    let env_idx = Select::new()
        .with_prompt("Environment")
        .items(envs)
        .default(0)
        .interact()?;

    let environment = match env_idx {
        0 => Environment::Sandbox,
        _ => Environment::Production,
    };
    let env_label = match environment {
        Environment::Sandbox => "sandbox",
        Environment::Production => "production",
    };

    println!();

    let consumer_key: String = Input::new()
        .with_prompt("Consumer Key")
        .interact_text()?;

    let consumer_secret: String = Password::new()
        .with_prompt("Consumer Secret")
        .interact()?;

    let passkey: String = Password::new()
        .with_prompt("Passkey")
        .interact()?;

    let shortcode: String = Input::new()
        .with_prompt("Shortcode")
        .default("174379".into())
        .interact_text()?;

    println!();

    let b2c = Confirm::new()
        .with_prompt("Set up B2C credentials? (initiator name + password)")
        .default(false)
        .interact()?;

    let (initiator_name, initiator_password) = if b2c {
        println!();
        let name: String = Input::new()
            .with_prompt("Initiator Name")
            .default("apitest".into())
            .interact_text()?;
        let pw: String = Password::new()
            .with_prompt("Initiator Password")
            .interact()?;
        (Some(name), Some(pw))
    } else {
        (None, None)
    };

    let credentials = Credentials {
        environment,
        consumer_key,
        consumer_secret,
        passkey,
        shortcode,
        initiator_name,
        initiator_password,
    };

    let key = config::generate_key();
    config::save_credentials(&key, &credentials)
        .context("failed to encrypt and save credentials")?;

    let sdk_key = config::key_to_string(&key, environment);

    term.write_line("").ok();
    term.write_line(
        &style("  ✓ Encrypted credentials saved to ~/.config/pesaraja/credentials.enc")
            .green()
            .to_string(),
    )
    .ok();
    term.write_line("").ok();
    term.write_line(&style("  Your SDK key:").bold().to_string()).ok();
    term.write_line("").ok();
    term.write_line(
        &style(format!("    {}", sdk_key))
            .yellow()
            .bold()
            .to_string(),
    )
    .ok();
    term.write_line("").ok();
    term.write_line(
        &style(format!(
            "  Add this to your .env for {}:",
            env_label
        ))
        .dim()
        .to_string(),
    )
    .ok();
    term.write_line(
        &style(format!("    DARAJA_KEY=\"{}\"", sdk_key))
            .dim()
            .to_string(),
    )
    .ok();
    term.write_line("").ok();

    Ok(())
}
