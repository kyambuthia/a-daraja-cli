//! Tests moved verbatim from the former `main.rs` monolith.
#[cfg(test)]
mod tests {
    use crate::common::{DEFAULT_RESULT_URL, DEFAULT_TIMEOUT_URL};
    use crate::creds::SANDBOX_PASSKEY;
    use crate::prelude::*;
    use daraja_rs::config::generate_sdk_key;
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

    fn test_credentials(shortcode: &str) -> Credentials {
        Credentials {
            consumer_key: Zeroizing::new("ck".into()),
            consumer_secret: Zeroizing::new("cs".into()),
            passkey: Zeroizing::new("pk".into()),
            shortcode: shortcode.into(),
            initiator_name: Some(Zeroizing::new("apitest".into())),
            initiator_password: Some(Zeroizing::new("pass".into())),
        }
    }

    fn sandbox_credentials(shortcode: &str, passkey: &str) -> Credentials {
        Credentials {
            consumer_key: Zeroizing::new("ck".into()),
            consumer_secret: Zeroizing::new("cs".into()),
            passkey: Zeroizing::new(passkey.into()),
            shortcode: shortcode.into(),
            initiator_name: Some(Zeroizing::new("apitest".into())),
            initiator_password: Some(Zeroizing::new("pass".into())),
        }
    }

    fn seed_profile_home(shortcode: &str, passkey: &str) -> tempfile::TempDir {
        let home = temp_home();
        let sdk_key = generate_sdk_key("test");
        let config_dir = home.path().join(".config/daraja");
        std::fs::create_dir_all(&config_dir).expect("create config dir");
        profiles::atomic_write_private(&config_dir.join("key"), sdk_key.as_bytes())
            .expect("write key");
        let creds = sandbox_credentials(shortcode, passkey);
        let profile = Profile {
            name: "personal".into(),
            profile_type: ProfileType::Personal,
            environment: Environment::Sandbox,
            merchant_number: shortcode.into(),
            credentials_path: profile_credentials_path("personal")
                .expect("profile credentials path")
                .display()
                .to_string(),
            initiator_configured: true,
        };

        save_profile(&profile, &creds, &sdk_key).expect("seed profile");
        home
    }

    #[test]
    fn personal_configuration_update_refreshes_profile_credentials() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let _home = temp_home();
        let sdk_key = generate_sdk_key("test");

        let first = test_credentials("174379");
        persist_personal_configuration("sandbox", &first, &sdk_key).expect("initial persist");

        let (active, loaded) = load_active_profile_context()
            .expect("load active context")
            .expect("active profile");
        assert_eq!(active.merchant_number, "174379");
        assert_eq!(loaded.shortcode, "174379");

        let updated = test_credentials("254700000001");
        persist_personal_configuration("sandbox", &updated, &sdk_key).expect("updated persist");

        let (active, loaded) = load_active_profile_context()
            .expect("reload active context")
            .expect("active profile after update");
        assert_eq!(active.merchant_number, "254700000001");
        assert_eq!(loaded.shortcode, "254700000001");
    }

    #[test]
    fn sandbox_async_urls_use_test_defaults_and_validate_them() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let home = temp_home();
        let config_dir = home.path().join(".config/daraja");
        std::fs::create_dir_all(&config_dir).expect("create config dir");
        profiles::atomic_write_private(
            &config_dir.join("key"),
            generate_sdk_key("test").as_bytes(),
        )
        .expect("write key");
        unsafe { std::env::remove_var("DARAJA_KEY") };

        let (result, timeout) = resolve_async_urls(None, None).expect("sandbox defaults");
        assert_eq!(result, DEFAULT_RESULT_URL);
        assert_eq!(timeout, DEFAULT_TIMEOUT_URL);
    }

    #[test]
    fn production_async_urls_require_explicit_values() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let home = temp_home();
        let config_dir = home.path().join(".config/daraja");
        std::fs::create_dir_all(&config_dir).expect("create config dir");
        profiles::atomic_write_private(
            &config_dir.join("key"),
            generate_sdk_key("live").as_bytes(),
        )
        .expect("write key");
        unsafe { std::env::remove_var("DARAJA_KEY") };

        let error = resolve_async_urls(None, None).expect_err("production defaults must fail");
        assert!(
            error
                .to_string()
                .contains("result and timeout URLs are required")
        );
    }

    #[test]
    fn callback_urls_are_validated_before_dry_run() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let home = temp_home();
        let config_dir = home.path().join(".config/daraja");
        std::fs::create_dir_all(&config_dir).expect("create config dir");
        profiles::atomic_write_private(
            &config_dir.join("key"),
            generate_sdk_key("test").as_bytes(),
        )
        .expect("write key");
        unsafe { std::env::remove_var("DARAJA_KEY") };

        let error = resolve_callback_url(Some("not-a-url".into()))
            .expect_err("invalid callback URL must fail");
        assert!(error.to_string().contains("callback URL"));
    }

    #[test]
    fn init_options_reject_phone_number_shortcodes() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let opts = InitOptions {
            env: Some(InitEnvironment::Sandbox),
            consumer_key: Some("ck".into()),
            shortcode: Some("0700000001".into()),
            initiator_name: None,
            non_interactive: true,
        };
        unsafe { std::env::set_var("DARAJA_CONSUMER_SECRET", "cs") };

        let err = match credentials_from_init_options(&opts) {
            Ok(_) => panic!("phone number should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("BusinessShortCode"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn init_options_accept_business_shortcodes() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let opts = InitOptions {
            env: Some(InitEnvironment::Sandbox),
            consumer_key: Some("ck".into()),
            shortcode: Some("174379".into()),
            initiator_name: None,
            non_interactive: true,
        };
        unsafe { std::env::set_var("DARAJA_CONSUMER_SECRET", "cs") };

        let creds = credentials_from_init_options(&opts).expect("business shortcode should pass");
        assert_eq!(creds.shortcode, "174379");
    }

    #[test]
    fn non_interactive_sandbox_init_uses_public_passkey() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let opts = InitOptions {
            env: Some(InitEnvironment::Sandbox),
            consumer_key: Some("ck".into()),
            shortcode: Some("174379".into()),
            initiator_name: None,
            non_interactive: true,
        };
        unsafe { std::env::set_var("DARAJA_CONSUMER_SECRET", "cs") };

        let creds = credentials_from_init_options(&opts)
            .expect("sandbox init should use the public passkey");
        assert_eq!(creds.passkey.as_str(), SANDBOX_PASSKEY);
    }

    #[test]
    fn sandbox_credentials_must_use_documented_passkey() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let creds = sandbox_credentials("174379", "wrong-passkey");
        let err = match validate_credentials_for_environment(Environment::Sandbox, &creds) {
            Ok(_) => panic!("sandbox passkey should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("documented sandbox passkey"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn sandbox_credentials_accept_documented_passkey() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let creds = sandbox_credentials("174379", SANDBOX_PASSKEY);
        validate_credentials_for_environment(Environment::Sandbox, &creds)
            .expect("documented sandbox passkey should be accepted");
    }

    #[test]
    fn validate_shortcode_rejects_msisdn_variants() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        for candidate in ["0700000001", "0112345678", "254700000001", "+254700000001"] {
            let err = match validate_shortcode(candidate.to_string()) {
                Ok(_) => panic!("mobile numbers should be rejected"),
                Err(err) => err,
            };
            assert!(
                err.to_string().contains("BusinessShortCode"),
                "unexpected error for {candidate}: {err}"
            );
        }
    }

    #[test]
    fn validate_shortcode_trims_and_accepts_business_shortcode() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let shortcode = validate_shortcode(" 174379 ".into()).expect("shortcode should pass");
        assert_eq!(shortcode, "174379");
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // Serializes process-wide HOME mutations for this test.
    async fn load_daraja_rejects_wrong_sandbox_passkey() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let _home = seed_profile_home("174379", "wrong-passkey");

        let err = match load_daraja().await {
            Ok(_) => panic!("wrong sandbox passkey should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("documented sandbox passkey"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn load_shortcode_reads_active_profile_shortcode() {
        let _guard = crate::TEST_ENV_LOCK.lock().expect("test env lock");
        let _home = seed_profile_home("174379", SANDBOX_PASSKEY);

        let shortcode = load_shortcode().expect("active profile shortcode");
        assert_eq!(shortcode, "174379");
    }

    #[test]
    fn base_url_accepts_loopback_http_and_removes_trailing_slash() {
        assert_eq!(
            validate_loopback_base_url("http://127.0.0.1:8787/").expect("loopback URL should pass"),
            "http://127.0.0.1:8787"
        );
        assert!(validate_loopback_base_url("http://localhost:8787").is_ok());
        assert!(validate_loopback_base_url("http://[::1]:8787").is_ok());
    }

    #[test]
    fn base_url_rejects_non_loopback_and_unsafe_urls() {
        for candidate in [
            "https://127.0.0.1:8787",
            "http://example.com:8787",
            "http://user:pass@127.0.0.1:8787",
            "http://127.0.0.1:8787/simulator",
            "http://127.0.0.1:8787/?token=secret",
        ] {
            assert!(
                validate_loopback_base_url(candidate).is_err(),
                "unsafe base URL should be rejected: {candidate}"
            );
        }
    }
}
