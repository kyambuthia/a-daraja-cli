use crate::client::DarajaClient;
use crate::error::DarajaError;
use crate::security::security_credential_for_environment;

/// RSA-encrypt the configured initiator password for B2C / status / balance / reversal APIs.
pub fn initiator_security_credential(client: &DarajaClient) -> Result<String, DarajaError> {
    let password = client
        .credentials
        .initiator_password
        .as_ref()
        .ok_or_else(|| DarajaError::Config("initiator_password is not configured".into()))?;

    security_credential_for_environment(password, client.environment())
}

/// Return the configured initiator name.
pub fn initiator_name(client: &DarajaClient) -> Result<String, DarajaError> {
    client
        .credentials
        .initiator_name
        .as_ref()
        .map(|name| name.to_string())
        .ok_or_else(|| DarajaError::Config("initiator_name is not configured".into()))
}
