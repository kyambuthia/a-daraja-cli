// Safaricom API certificates for SecurityCredential RSA encryption.
//
// Certificates are never bundled in the repo. Sandbox cert is downloaded during
// `pesaraja init`; production cert comes from the Daraja portal.

use crate::error::DarajaError;
use crate::types::Environment;
use std::path::{Path, PathBuf};

/// Official Safaricom sandbox API certificate (public key, not a secret).
pub const SANDBOX_CERT_URL: &str =
    "https://raw.githubusercontent.com/safaricom/mpesa-node-library/master/keys/sandbox-cert.cer";

/// Return the PEM certificate for the given Daraja environment.
pub fn certificate_pem(environment: Environment) -> Result<String, DarajaError> {
    let path = certificate_path(environment)?;
    std::fs::read_to_string(&path).map_err(|e| missing_certificate_error(environment, &path, e))
}

/// Path where the certificate for this environment should be stored.
pub fn certificate_path(environment: Environment) -> Result<PathBuf, DarajaError> {
    if let Ok(path) = std::env::var("DARAJA_CERT_PATH") {
        return Ok(PathBuf::from(path));
    }

    match environment {
        Environment::Sandbox => {
            if let Ok(path) = std::env::var("DARAJA_SANDBOX_CERT_PATH") {
                return Ok(PathBuf::from(path));
            }
            Ok(daraja_config_dir()?.join("sandbox.cer"))
        }
        Environment::Production => Ok(daraja_config_dir()?.join("production.cer")),
    }
}

/// Download the sandbox certificate if it is not already on disk.
pub async fn ensure_sandbox_certificate() -> Result<PathBuf, DarajaError> {
    let path = certificate_path(Environment::Sandbox)?;
    if path.exists() {
        return Ok(path);
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            DarajaError::Config(format!(
                "failed to create certificate directory at {}: {e}",
                parent.display()
            ))
        })?;
    }

    let pem = reqwest::get(SANDBOX_CERT_URL)
        .await
        .map_err(|e| DarajaError::Network(format!("failed to download sandbox certificate: {e}")))?
        .error_for_status()
        .map_err(|e| {
            DarajaError::Network(format!(
                "failed to download sandbox certificate from {SANDBOX_CERT_URL}: {e}"
            ))
        })?
        .text()
        .await
        .map_err(|e| DarajaError::Network(format!("failed to read sandbox certificate: {e}")))?;

    validate_certificate_pem(&pem)?;

    std::fs::write(&path, &pem).map_err(|e| {
        DarajaError::Config(format!(
            "failed to write sandbox certificate to {}: {e}",
            path.display()
        ))
    })?;

    Ok(path)
}

fn daraja_config_dir() -> Result<PathBuf, DarajaError> {
    let home = std::env::var("HOME")
        .map_err(|_| DarajaError::Config("HOME environment variable is not set".into()))?;
    Ok(PathBuf::from(format!("{home}/.config/daraja")))
}

fn validate_certificate_pem(pem: &str) -> Result<(), DarajaError> {
    let pem = pem.trim();
    if pem.contains("BEGIN CERTIFICATE") && pem.contains("END CERTIFICATE") {
        Ok(())
    } else {
        Err(DarajaError::Config(
            "downloaded sandbox certificate is not valid PEM".into(),
        ))
    }
}

fn missing_certificate_error(
    environment: Environment,
    path: &Path,
    error: std::io::Error,
) -> DarajaError {
    let hint = match environment {
        Environment::Sandbox => {
            format!(
                "sandbox certificate not found at {}: {error}. \
                 Run `pesaraja init` to download it, or set DARAJA_SANDBOX_CERT_PATH.",
                path.display()
            )
        }
        Environment::Production => {
            format!(
                "production certificate not found at {}: {error}. \
                 Download the Safaricom API certificate from the Daraja portal and save it there, \
                 or set DARAJA_CERT_PATH.",
                path.display()
            )
        }
    };
    DarajaError::Config(hint)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_temp_cert(dir: &tempfile::TempDir) -> PathBuf {
        use rand::rngs::OsRng;
        use rsa::{pkcs8::EncodePublicKey, RsaPrivateKey};

        let mut rng = OsRng;
        let private_key = RsaPrivateKey::new(&mut rng, 2048).expect("RSA key generation");
        let pem = private_key
            .to_public_key()
            .to_public_key_pem(rsa::pkcs8::LineEnding::LF)
            .expect("public key PEM encoding");

        let path = dir.path().join("sandbox.cer");
        std::fs::write(&path, pem).expect("write temp cert");
        path
    }

    #[test]
    fn certificate_pem_reads_from_daraja_cert_path_override() {
        let dir = tempfile::tempdir().expect("temp dir");
        let cert_path = write_temp_cert(&dir);

        unsafe { std::env::set_var("DARAJA_CERT_PATH", cert_path.to_str().unwrap()) };

        let pem = certificate_pem(Environment::Sandbox).expect("read certificate");
        assert!(pem.contains("BEGIN"));

        unsafe { std::env::remove_var("DARAJA_CERT_PATH") };
    }

    #[test]
    fn validate_certificate_pem_rejects_garbage() {
        let result = validate_certificate_pem("not a certificate");
        assert!(result.is_err());
    }
}