use daraja_rs::config::{decrypt_credentials, encrypt_credentials, extract_key_from_sdk_key, Credentials};
use daraja_rs::error::DarajaError;
use daraja_rs::types::Environment;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileType {
    Personal,
    Paybill,
    Till,
    Pochi,
}

impl ProfileType {
    pub fn as_str(self) -> &'static str {
        match self {
            ProfileType::Personal => "personal",
            ProfileType::Paybill => "paybill",
            ProfileType::Till => "till",
            ProfileType::Pochi => "pochi",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub profile_type: ProfileType,
    pub environment: Environment,
    pub merchant_number: String,
    pub credentials_path: String,
    pub initiator_configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileRegistry {
    pub active_profile: Option<String>,
    pub profiles: Vec<Profile>,
}

fn config_dir() -> Result<PathBuf, DarajaError> {
    let home = std::env::var("HOME")
        .map_err(|_| DarajaError::Config("HOME environment variable is not set".into()))?;
    Ok(PathBuf::from(format!("{home}/.config/daraja")))
}

pub fn profiles_registry_path() -> Result<PathBuf, DarajaError> {
    Ok(config_dir()?.join("profiles.json"))
}

pub fn legacy_credentials_path() -> Result<PathBuf, DarajaError> {
    Ok(config_dir()?.join("credentials.enc"))
}

pub fn legacy_sdk_key_path() -> Result<PathBuf, DarajaError> {
    Ok(config_dir()?.join("key"))
}

pub fn profile_credentials_path(name: &str) -> Result<PathBuf, DarajaError> {
    Ok(config_dir()?.join("profiles").join(format!("{name}.enc")))
}

pub fn save_profile(
    profile: &Profile,
    credentials: &Credentials,
    sdk_key: &str,
) -> Result<(), DarajaError> {
    let key_bytes = extract_key_from_sdk_key(sdk_key)?;
    let encrypted = encrypt_credentials(credentials, &key_bytes)?;
    let creds_path = PathBuf::from(&profile.credentials_path);
    if let Some(parent) = creds_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            DarajaError::Config(format!("failed to create profile directory: {e}"))
        })?;
    }
    std::fs::write(&creds_path, encrypted).map_err(|e| {
        DarajaError::Config(format!(
            "failed to write profile credentials to {}: {e}",
            creds_path.display()
        ))
    })?;

    let mut registry = load_registry().unwrap_or_default();
    registry.profiles.retain(|p| p.name != profile.name);
    registry.profiles.push(profile.clone());
    registry.active_profile = Some(profile.name.clone());
    save_registry(&registry)
}

pub fn load_registry() -> Result<ProfileRegistry, DarajaError> {
    let path = profiles_registry_path()?;
    if path.exists() {
        let text = std::fs::read_to_string(&path).map_err(|e| {
            DarajaError::Config(format!("failed to read profile registry: {e}"))
        })?;
        return serde_json::from_str(&text)
            .map_err(|e| DarajaError::Config(format!("failed to parse profile registry: {e}")));
    }

    legacy_registry()
}

pub fn save_registry(registry: &ProfileRegistry) -> Result<(), DarajaError> {
    let path = profiles_registry_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            DarajaError::Config(format!("failed to create config directory: {e}"))
        })?;
    }
    let text = serde_json::to_string_pretty(registry)
        .map_err(|e| DarajaError::Config(format!("failed to serialize profile registry: {e}")))?;
    std::fs::write(&path, text).map_err(|e| {
        DarajaError::Config(format!("failed to write profile registry: {e}"))
    })
}

pub fn legacy_registry() -> Result<ProfileRegistry, DarajaError> {
    let creds_path = legacy_credentials_path()?;
    let key_path = legacy_sdk_key_path()?;
    if !creds_path.exists() || !key_path.exists() {
        return Ok(ProfileRegistry::default());
    }

    let sdk_key = std::fs::read_to_string(&key_path)
        .map_err(|e| DarajaError::Config(format!("failed to read legacy SDK key: {e}")))?;
    let key_bytes = extract_key_from_sdk_key(sdk_key.trim())?;
    let encrypted = std::fs::read(&creds_path)
        .map_err(|e| DarajaError::Config(format!("failed to read legacy credentials: {e}")))?;
    let creds = decrypt_credentials(&encrypted, &key_bytes)?;
    let creds_path = profile_credentials_path("personal")?;

    let profile = Profile {
        name: "personal".into(),
        profile_type: ProfileType::Personal,
        environment: if sdk_key.trim().starts_with("dsk_test_") {
            Environment::Sandbox
        } else {
            Environment::Production
        },
        merchant_number: creds.shortcode,
        credentials_path: creds_path.display().to_string(),
        initiator_configured: creds.initiator_name.is_some() && creds.initiator_password.is_some(),
    };

    Ok(ProfileRegistry {
        active_profile: Some(profile.name.clone()),
        profiles: vec![profile],
    })
}

pub fn active_profile<'a>(registry: &'a ProfileRegistry) -> Option<&'a Profile> {
    let name = registry.active_profile.as_deref()?;
    registry.profiles.iter().find(|profile| profile.name == name)
}

pub fn profile_summary(profile: &Profile) -> serde_json::Value {
    serde_json::json!({
        "name": profile.name,
        "type": profile.profile_type.as_str(),
        "environment": match profile.environment {
            Environment::Sandbox => "sandbox",
            Environment::Production => "production",
        },
        "merchant_number": profile.merchant_number,
        "credentials_path": profile.credentials_path,
        "initiator_configured": profile.initiator_configured,
    })
}

pub fn load_profile_credentials(profile: &Profile) -> Result<Credentials, DarajaError> {
    let key_path = legacy_sdk_key_path()?;
    let sdk_key = std::fs::read_to_string(&key_path)
        .map_err(|e| DarajaError::Config(format!("failed to read SDK key: {e}")))?;
    let key_bytes = extract_key_from_sdk_key(sdk_key.trim())?;
    let creds_path = PathBuf::from(&profile.credentials_path);
    let encrypted = std::fs::read(&creds_path).map_err(|e| {
        DarajaError::Config(format!(
            "failed to read profile credentials at {}: {e}",
            creds_path.display()
        ))
    })?;
    decrypt_credentials(&encrypted, &key_bytes)
}

pub fn load_active_profile_context() -> Result<Option<(Profile, Credentials)>, DarajaError> {
    let registry = load_registry()?;
    let profile = match active_profile(&registry) {
        Some(profile) => profile.clone(),
        None => return Ok(None),
    };
    let credentials = load_profile_credentials(&profile)?;
    Ok(Some((profile, credentials)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use daraja_rs::config::{encrypt_credentials, generate_sdk_key};
    use zeroize::Zeroizing;

    fn temp_home() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp dir");
        unsafe { std::env::set_var("HOME", dir.path()) };
        dir
    }

    #[test]
    fn profile_type_as_str_is_stable() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        assert_eq!(ProfileType::Personal.as_str(), "personal");
        assert_eq!(ProfileType::Paybill.as_str(), "paybill");
        assert_eq!(ProfileType::Till.as_str(), "till");
        assert_eq!(ProfileType::Pochi.as_str(), "pochi");
    }

    #[test]
    fn empty_registry_is_default_when_no_legacy_files_exist() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let _home = temp_home();
        let registry = load_registry().expect("load registry");
        assert!(registry.active_profile.is_none());
        assert!(registry.profiles.is_empty());
    }

    #[test]
    fn save_and_load_registry_round_trips() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let _home = temp_home();
        let registry = ProfileRegistry {
            active_profile: Some("personal".into()),
            profiles: vec![Profile {
                name: "personal".into(),
                profile_type: ProfileType::Personal,
                environment: Environment::Sandbox,
                merchant_number: "254743121299".into(),
                credentials_path: "/tmp/personal.enc".into(),
                initiator_configured: false,
            }],
        };
        save_registry(&registry).expect("save registry");
        let loaded = load_registry().expect("load registry");
        assert_eq!(loaded.active_profile, Some("personal".into()));
        assert_eq!(loaded.profiles.len(), 1);
        assert_eq!(loaded.profiles[0].merchant_number, "254743121299");
    }

    #[test]
    fn legacy_files_migrate_to_personal_profile() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let home = temp_home();
        let config_dir = home.path().join(".config/daraja");
        std::fs::create_dir_all(&config_dir).expect("create config dir");

        let sdk_key = generate_sdk_key("test");
        let key_path = config_dir.join("key");
        std::fs::write(&key_path, &sdk_key).expect("write legacy key");

        let creds = Credentials {
            consumer_key: Zeroizing::new("ck".into()),
            consumer_secret: Zeroizing::new("cs".into()),
            passkey: Zeroizing::new("pk".into()),
            shortcode: "254743121299".into(),
            initiator_name: Some(Zeroizing::new("apitest".into())),
            initiator_password: Some(Zeroizing::new("pass".into())),
        };

        let key_bytes = daraja_rs::config::extract_key_from_sdk_key(&sdk_key).expect("key bytes");
        let encrypted = encrypt_credentials(&creds, &key_bytes).expect("encrypt creds");
        std::fs::write(config_dir.join("credentials.enc"), encrypted).expect("write legacy creds");

        let registry = load_registry().expect("load migrated registry");
        assert_eq!(registry.active_profile, Some("personal".into()));
        assert_eq!(registry.profiles.len(), 1);
        let profile = &registry.profiles[0];
        assert_eq!(profile.name, "personal");
        assert_eq!(profile.profile_type.as_str(), "personal");
        assert_eq!(profile.merchant_number, "254743121299");
    }

    #[test]
    fn active_profile_context_loads_profile_credentials() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let home = temp_home();
        let config_dir = home.path().join(".config/daraja");
        std::fs::create_dir_all(config_dir.join("profiles")).expect("create profile dir");

        let sdk_key = generate_sdk_key("test");
        std::fs::write(config_dir.join("key"), &sdk_key).expect("write key");

        let creds = Credentials {
            consumer_key: Zeroizing::new("ck".into()),
            consumer_secret: Zeroizing::new("cs".into()),
            passkey: Zeroizing::new("pk".into()),
            shortcode: "254743121299".into(),
            initiator_name: Some(Zeroizing::new("apitest".into())),
            initiator_password: Some(Zeroizing::new("pass".into())),
        };
        let profile = Profile {
            name: "personal".into(),
            profile_type: ProfileType::Personal,
            environment: Environment::Sandbox,
            merchant_number: "254743121299".into(),
            credentials_path: config_dir.join("profiles/personal.enc").display().to_string(),
            initiator_configured: true,
        };
        save_profile(&profile, &creds, &sdk_key).expect("save profile");

        let (active, loaded) = load_active_profile_context()
            .expect("load active context")
            .expect("active context");
        assert_eq!(active.name, "personal");
        assert_eq!(loaded.shortcode, "254743121299");
        assert_eq!(loaded.consumer_key.as_str(), "ck");
    }
}
