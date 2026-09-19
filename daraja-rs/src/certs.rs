// Safaricom API certificates for SecurityCredential RSA encryption.
//
// Certificates are never bundled in the repo. Sandbox cert is downloaded during
// `pesaraja init`; production cert comes from the Daraja portal.

use crate::config::daraja_config_dir;
use crate::error::DarajaError;
use crate::types::Environment;
use sha2::Digest;
use std::path::{Path, PathBuf};
use x509_cert::der::DecodePem;

/// Official Safaricom sandbox API certificate (public key, not a secret).
pub const SANDBOX_CERT_URL: &str = "https://raw.githubusercontent.com/safaricom/mpesa-node-library/678e3c33b20c1447c209eb88eded89f1bb751e11/keys/sandbox-cert.cer";
pub const SANDBOX_CERT_SHA256: &str =
    "bc97ac696be7c030d71631508b325fc03b335342d88d5ec466c7ffc368bc4550";

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
    let digest = format!("{:x}", sha2::Sha256::digest(pem.as_bytes()));
    if digest != SANDBOX_CERT_SHA256 {
        return Err(DarajaError::Config(format!(
            "downloaded sandbox certificate checksum mismatch (got {digest})"
        )));
    }

    std::fs::write(&path, &pem).map_err(|e| {
        DarajaError::Config(format!(
            "failed to write sandbox certificate to {}: {e}",
            path.display()
        ))
    })?;

    Ok(path)
}

fn validate_certificate_pem(pem: &str) -> Result<(), DarajaError> {
    let pem = pem.trim();
    if !pem.contains("BEGIN CERTIFICATE") || !pem.contains("END CERTIFICATE") {
        return Err(DarajaError::Config(
            "downloaded sandbox certificate is not valid PEM".into(),
        ));
    }
    x509_cert::Certificate::from_pem(pem)
        .map(|_| ())
        .map_err(|_| {
            DarajaError::Config("downloaded sandbox certificate is not valid X.509".into())
        })
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
        use rsa::{RsaPrivateKey, pkcs8::EncodePublicKey};

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
