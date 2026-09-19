use daraja_rs::error::DarajaError;
use serde::Serialize;
use std::process::ExitCode;

pub fn print_json<T: Serialize>(value: &T) -> Result<(), DarajaError> {
    let rendered = serde_json::to_string_pretty(value)
        .map_err(|error| DarajaError::Config(format!("serialize JSON output: {error}")))?;
    println!("{rendered}");
    Ok(())
}

pub fn print_error(error: &DarajaError, json: bool) {
    if json {
        let value = serde_json::json!({
            "error": {
                "code": error.code(),
                "message": error.to_string(),
                "outcomeUnknown": error.is_ambiguous_outcome(),
            }
        });
        match serde_json::to_string(&value) {
            Ok(rendered) => eprintln!("{rendered}"),
            Err(_) => eprintln!("{{\"error\":{{\"code\":\"output_error\"}}}}"),
        }
    } else {
        eprintln!("× {error}");
    }
}

pub fn exit_code(error: &DarajaError) -> ExitCode {
    let code = match error {
        DarajaError::Config(_)
        | DarajaError::Encryption(_)
        | DarajaError::MissingApiKey
        | DarajaError::WrongEnvironmentPrefix { .. }
        | DarajaError::MissingIdempotencyKey => 2,
        DarajaError::Api { .. } | DarajaError::AuthFailed(_) => 3,
        DarajaError::RequestOutcomeUnknown(_)
        | DarajaError::LocalStateUncertain(_)
        | DarajaError::IdempotencyOutcomeUnknown => 4,
        _ => 1,
    };
    ExitCode::from(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_outcomes_have_a_distinct_exit_code() {
        let error = DarajaError::RequestOutcomeUnknown("timeout".into());
        assert_eq!(exit_code(&error), ExitCode::from(4));
    }
}
