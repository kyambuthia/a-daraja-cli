// Safaricom API certificates for SecurityCredential RSA encryption.
//
// Sandbox certificate is bundled. Production certificate is read from
// ~/.config/daraja/production.cer (download from the Daraja portal on Go Live).

use crate::error::DarajaError;
use crate::types::Environment;
use std::path::PathBuf;

const SANDBOX_CERT: &str = include_str!("../certs/sandbox.cer");

/// Return the PEM certificate for the given Daraja environment.
pub fn certificate_pem(environment: Environment) -> Result<String, DarajaError> {
    match environment {
        Environment::Sandbox => Ok(SANDBOX_CERT.to_string()),
        Environment::Production => load_production_certificate(),
    }
}

fn production_cert_path() -> Result<PathBuf, DarajaError> {
    if let Ok(path) = std::env::var("DARAJA_CERT_PATH") {
        return Ok(PathBuf::from(path));
    }

    let home = std::env::var("HOME")
        .map_err(|_| DarajaError::Config("HOME environment variable is not set".into()))?;
    Ok(PathBuf::from(format!(
        "{home}/.config/daraja/production.cer"
    )))
}

fn load_production_certificate() -> Result<String, DarajaError> {
    let path = production_cert_path()?;
    std::fs::read_to_string(&path).map_err(|e| {
        DarajaError::Config(format!(
            "production certificate not found at {}: {e}. \
             Download the Safaricom API certificate from the Daraja portal and save it there, \
             or set DARAJA_CERT_PATH.",
            path.display()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_certificate_is_valid_pem() {
        let pem = certificate_pem(Environment::Sandbox).unwrap();
        assert!(pem.contains("BEGIN CERTIFICATE"));
        assert!(pem.contains("END CERTIFICATE"));
    }

    #[test]
    fn sandbox_certificate_parses_for_security_credential() {
        use crate::security::generate_security_credential;

        let pem = certificate_pem(Environment::Sandbox).unwrap();
        let credential =
            generate_security_credential("Safaricom999!*!", &pem).expect("credential generation");
        assert!(!credential.is_empty());
    }
}