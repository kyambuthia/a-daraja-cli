use daraja_rs::config::{
    Credentials, daraja_config_dir, decrypt_credentials, encrypt_credentials,
    extract_key_from_sdk_key,
};
use daraja_rs::error::DarajaError;
use daraja_rs::types::Environment;
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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
    daraja_config_dir()
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
    validate_profile_name(name)?;
    Ok(config_dir()?.join("profiles").join(format!("{name}.enc")))
}

pub fn validate_profile_name(name: &str) -> Result<(), DarajaError> {
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(DarajaError::Config(
            "profile name must be 1-64 ASCII letters, numbers, '.', '_' or '-'".into(),
        ));
    }
    Ok(())
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
        std::fs::create_dir_all(parent)
            .map_err(|e| DarajaError::Config(format!("failed to create profile directory: {e}")))?;
    }
    atomic_write_private(&creds_path, &encrypted).map_err(|e| {
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
        ensure_private_permissions(&path)?;
        let text = std::fs::read_to_string(&path)
            .map_err(|e| DarajaError::Config(format!("failed to read profile registry: {e}")))?;
        return serde_json::from_str(&text)
            .map_err(|e| DarajaError::Config(format!("failed to parse profile registry: {e}")));
    }

    legacy_registry()
}

pub fn save_registry(registry: &ProfileRegistry) -> Result<(), DarajaError> {
    let path = profiles_registry_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| DarajaError::Config(format!("failed to create config directory: {e}")))?;
    }
    let text = serde_json::to_string_pretty(registry)
        .map_err(|e| DarajaError::Config(format!("failed to serialize profile registry: {e}")))?;
    atomic_write_private(&path, text.as_bytes())
        .map_err(|e| DarajaError::Config(format!("failed to write profile registry: {e}")))
}

/// Replace a private configuration file without exposing a partial write.
pub fn atomic_write_private(path: &Path, contents: &[u8]) -> Result<(), DarajaError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| DarajaError::Config(format!("failed to create config directory: {e}")))?;
        set_private_directory_permissions(parent)?;
    }

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            DarajaError::Config(format!("invalid configuration path: {}", path.display()))
        })?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = path.with_file_name(format!(".{file_name}.tmp-{}-{nonce}", std::process::id()));

    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| DarajaError::Config(format!("failed to create temporary file: {e}")))?;
        use std::io::Write;
        file.write_all(contents)
            .map_err(|e| DarajaError::Config(format!("failed to write temporary file: {e}")))?;
        file.sync_all()
            .map_err(|e| DarajaError::Config(format!("failed to sync temporary file: {e}")))?;
        set_private_permissions(&temporary)?;
        std::fs::rename(&temporary, path).map_err(|e| {
            DarajaError::Config(format!("failed to replace configuration file: {e}"))
        })?;
        Ok(())
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

#[cfg(unix)]
fn set_private_permissions(path: &std::path::Path) -> Result<(), DarajaError> {
    let mut perms = std::fs::metadata(path)
        .map_err(|e| DarajaError::Config(format!("failed to read file metadata: {e}")))?
        .permissions();
    perms.set_mode(0o600);
    std::fs::set_permissions(path, perms)
        .map_err(|e| DarajaError::Config(format!("failed to set private file permissions: {e}")))
}

#[cfg(not(unix))]
fn set_private_permissions(_path: &std::path::Path) -> Result<(), DarajaError> {
    Ok(())
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &std::path::Path) -> Result<(), DarajaError> {
    let mut perms = std::fs::metadata(path)
        .map_err(|e| DarajaError::Config(format!("failed to read directory metadata: {e}")))?
        .permissions();
    perms.set_mode(0o700);
    std::fs::set_permissions(path, perms)
        .map_err(|e| DarajaError::Config(format!("failed to set directory permissions: {e}")))
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &std::path::Path) -> Result<(), DarajaError> {
    Ok(())
}

/// Refuse to read a private configuration file that is accessible to other
/// users on Unix. This is intentionally a fail-closed check.
pub fn ensure_private_permissions(path: &Path) -> Result<(), DarajaError> {
    let metadata = std::fs::metadata(path).map_err(|error| {
        DarajaError::Config(format!(
            "read private file metadata at {}: {error}",
            path.display()
        ))
    })?;
    #[cfg(unix)]
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(DarajaError::Config(format!(
            "private file {} is accessible to other users; set mode 0600",
            path.display()
        )));
    }
    Ok(())
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

pub fn active_profile(registry: &ProfileRegistry) -> Option<&Profile> {
    let name = std::env::var("PESARAJA_PROFILE")
        .ok()
        .or_else(|| registry.active_profile.clone())?;
    registry
        .profiles
        .iter()
        .find(|profile| profile.name == name)
}

pub fn select_profile(name: &str) -> Result<(), DarajaError> {
    validate_profile_name(name)?;
    let mut registry = load_registry()?;
    if !registry.profiles.iter().any(|profile| profile.name == name) {
        return Err(DarajaError::Config(format!(
            "profile '{name}' was not found"
        )));
    }
    registry.active_profile = Some(name.to_string());
    save_registry(&registry)
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
    let sdk_key = if let Ok(key) = std::env::var("DARAJA_KEY")
        && !key.trim().is_empty()
    {
        key
    } else {
        ensure_private_permissions(&key_path)?;
        std::fs::read_to_string(&key_path)
            .map_err(|e| DarajaError::Config(format!("failed to read SDK key: {e}")))?
    };
    let key_bytes = extract_key_from_sdk_key(sdk_key.trim())?;
    let key_environment = if sdk_key.trim().starts_with("dsk_test_") {
        Environment::Sandbox
    } else {
        Environment::Production
    };
    if key_environment != profile.environment {
        return Err(DarajaError::WrongEnvironmentPrefix {
            expected: match profile.environment {
                Environment::Sandbox => "dsk_test_".into(),
                Environment::Production => "dsk_live_".into(),
            },
            actual: match key_environment {
                Environment::Sandbox => "dsk_test_".into(),
                Environment::Production => "dsk_live_".into(),
            },
        });
    }
    let creds_path = PathBuf::from(&profile.credentials_path);
    ensure_private_permissions(&creds_path)?;
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
        unsafe {
            std::env::set_var("HOME", dir.path());
            // `XDG_CONFIG_HOME` and `DARAJA_CONFIG_DIR` both take precedence over
            // `HOME`, so isolate them too or CI's pre-set XDG_CONFIG_HOME leaks in.
            std::env::set_var("XDG_CONFIG_HOME", dir.path().join(".config"));
            std::env::remove_var("DARAJA_CONFIG_DIR");
        }
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
    fn profile_names_reject_path_traversal() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        for name in ["../other", "profile/name", ""] {
            assert!(validate_profile_name(name).is_err(), "accepted {name}");
        }
        let too_long = "a".repeat(65);
        assert!(validate_profile_name(&too_long).is_err());
        validate_profile_name("merchant-prod_1").expect("valid profile name");
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
    fn select_profile_updates_active_profile() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let _home = temp_home();
        let registry = ProfileRegistry {
            active_profile: Some("personal".into()),
            profiles: vec![
                Profile {
                    name: "personal".into(),
                    profile_type: ProfileType::Personal,
                    environment: Environment::Sandbox,
                    merchant_number: "174379".into(),
                    credentials_path: "/tmp/personal.enc".into(),
                    initiator_configured: false,
                },
                Profile {
                    name: "shop".into(),
                    profile_type: ProfileType::Till,
                    environment: Environment::Sandbox,
                    merchant_number: "123456".into(),
                    credentials_path: "/tmp/shop.enc".into(),
                    initiator_configured: false,
                },
            ],
        };
        save_registry(&registry).expect("save registry");
        select_profile("shop").expect("select profile");
        let loaded = load_registry().expect("load registry");
        assert_eq!(loaded.active_profile.as_deref(), Some("shop"));
    }

    #[test]
    fn private_configuration_writes_replace_files_atomically() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let home = temp_home();
        let path = home.path().join(".config/daraja/key");

        atomic_write_private(&path, b"first").expect("write first value");
        atomic_write_private(&path, b"second").expect("replace value");

        assert_eq!(
            std::fs::read_to_string(&path).expect("read value"),
            "second"
        );
        let temporary_files = std::fs::read_dir(path.parent().expect("parent"))
            .expect("read config directory")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".key.tmp-"))
            .count();
        assert_eq!(temporary_files, 0);
        let mode = std::fs::metadata(path)
            .expect("key metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn private_permission_check_rejects_group_or_world_access() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let home = temp_home();
        let path = home.path().join(".config/daraja/key");
        atomic_write_private(&path, b"secret").expect("write private file");
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o644);
        std::fs::set_permissions(&path, permissions).unwrap();
        assert!(ensure_private_permissions(&path).is_err());
    }

    #[test]
    fn legacy_files_migrate_to_personal_profile() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let home = temp_home();
        let config_dir = home.path().join(".config/daraja");
        std::fs::create_dir_all(&config_dir).expect("create config dir");

        let sdk_key = generate_sdk_key("test");
        let key_path = config_dir.join("key");
        atomic_write_private(&key_path, sdk_key.as_bytes()).expect("write legacy key");

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
        atomic_write_private(&config_dir.join("credentials.enc"), &encrypted)
            .expect("write legacy creds");

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
        atomic_write_private(&config_dir.join("key"), sdk_key.as_bytes()).expect("write key");

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
            credentials_path: config_dir
                .join("profiles/personal.enc")
                .display()
                .to_string(),
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
