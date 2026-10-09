use crate::prelude::*;

pub(crate) struct CheckItem {
    ok: bool,
    /// Whether failing this check means the CLI cannot work at all.
    ///
    /// Carried on the check rather than inferred from its wording: the exit
    /// status used to be decided by substring-matching the messages, so
    /// rewording one silently changed whether `doctor` failed.
    critical: bool,
    message: String,
    hint: Option<String>,
}

pub(crate) fn print_check(item: &CheckItem) {
    let mark = if item.ok { "✓" } else { "✗" };
    println!("{mark} {}", item.message);
    if let Some(hint) = &item.hint {
        println!("  {hint}");
    }
}

pub(crate) async fn cmd_doctor(json: bool) -> Result<(), DarajaError> {
    let creds_path = credentials_path()?;
    let creds_exist = creds_path.exists();
    let daraja_key = load_sdk_key().ok();
    let key_set = daraja_key.is_some();
    let key_from_env = std::env::var("DARAJA_KEY").ok().is_some();

    let mut checks = stored_secret_checks(&creds_path, creds_exist, key_set, key_from_env)?;

    let is_sandbox = daraja_key
        .as_deref()
        .map(|k| k.starts_with("dsk_test_"))
        .unwrap_or(false);
    let is_production = daraja_key
        .as_deref()
        .map(|k| k.starts_with("dsk_live_"))
        .unwrap_or(false);

    if let Some(key) = &daraja_key {
        if !is_sandbox && !is_production {
            checks.push(CheckItem {
                ok: false,
                critical: true,
                message: "SDK key has an invalid prefix (expected dsk_test_ or dsk_live_)".into(),
                hint: Some("Re-run `pesaraja init` to generate a valid key.".into()),
            });
        } else if creds_exist {
            match config::extract_key_from_sdk_key(key) {
                Ok(key_bytes) => {
                    let encrypted = std::fs::read(&creds_path).map_err(|e| {
                        DarajaError::Config(format!("failed to read credentials: {e}"))
                    })?;
                    match config::decrypt_credentials(&encrypted, &key_bytes) {
                        Ok(creds) => {
                            let env = if is_sandbox {
                                Environment::Sandbox
                            } else {
                                Environment::Production
                            };
                            let env_name = if is_sandbox { "Sandbox" } else { "Production" };

                            let mut token_manager = TokenManager::new(
                                creds.consumer_key.to_string(),
                                creds.consumer_secret.to_string(),
                                env,
                            );
                            match token_manager.get_token().await {
                                Ok(token) => {
                                    let expires_in =
                                        (token.expires_at - chrono::Utc::now()).num_seconds();
                                    let expires_in = expires_in.max(0);
                                    checks.push(CheckItem {
                                        ok: true,
                                        critical: false,
                                        message: format!(
                                            "{env_name} OAuth token: valid (expires in {expires_in}s)"
                                        ),
                                        hint: None,
                                    });
                                }
                                Err(err) => {
                                    checks.push(CheckItem {
                                        ok: false,
                                        critical: true,
                                        message: format!("{env_name} OAuth failed: {err}"),
                                        hint: Some(
                                            "Check your consumer key and secret in `pesaraja init`."
                                                .into(),
                                        ),
                                    });
                                }
                            }

                            let has_initiator = creds.initiator_name.is_some()
                                && creds.initiator_password.is_some();
                            if has_initiator {
                                checks.push(CheckItem {
                                    ok: true,
                                    critical: false,
                                    message: "Initiator credentials configured".into(),
                                    hint: None,
                                });
                            } else {
                                checks.push(CheckItem {
                                    ok: false,
                                    critical: false,
                                    message: "Initiator credentials not configured".into(),
                                    hint: Some(
                                        "Required for send, query, balance, and refund. \
                                         Re-run `pesaraja init` and add initiator details."
                                            .into(),
                                    ),
                                });
                            }

                            if is_sandbox {
                                if let Err(err) = validate_credentials_for_environment(
                                    Environment::Sandbox,
                                    &creds,
                                ) {
                                    checks.push(CheckItem {
                                        ok: false,
                                        critical: true,
                                        message: format!("Sandbox passkey mismatch: {err}"),
                                        hint: Some(
                                            "Re-run `pesaraja init` and use the public sandbox passkey."
                                                .into(),
                                        ),
                                    });
                                } else {
                                    checks.push(CheckItem {
                                        ok: true,
                                        critical: false,
                                        message: "Sandbox passkey matches the documented value"
                                            .into(),
                                        hint: None,
                                    });
                                }
                            }

                            if is_sandbox {
                                checks.push(CheckItem {
                                    ok: false,
                                    critical: false,
                                    message: "send: not available in sandbox (needs Go Live)"
                                        .into(),
                                    hint: None,
                                });
                            } else if has_initiator {
                                checks.push(CheckItem {
                                    ok: true,
                                    critical: false,
                                    message: "send: credentials ready".into(),
                                    hint: None,
                                });
                            }
                        }
                        Err(err) => {
                            checks.push(CheckItem {
                                ok: false,
                                critical: true,
                                message: format!("Failed to decrypt credentials: {err}"),
                                hint: Some(
                                    "Your DARAJA_KEY may not match the encrypted file.".into(),
                                ),
                            });
                        }
                    }
                }
                Err(err) => {
                    checks.push(CheckItem {
                        ok: false,
                        critical: true,
                        message: format!("Invalid SDK key format: {err}"),
                        hint: None,
                    });
                }
            }
        }
    }

    if is_sandbox {
        checks.push(CheckItem {
            ok: true,
            critical: false,
            message: "Using sandbox environment (dsk_test_)".into(),
            hint: None,
        });

        let sandbox_cert_path = certificate_path(Environment::Sandbox)?;
        let sandbox_cert_ok = sandbox_cert_path.exists();
        checks.push(CheckItem {
            ok: sandbox_cert_ok,
            critical: false,
            message: if sandbox_cert_ok {
                "Sandbox API certificate found".into()
            } else {
                "Sandbox API certificate not found".into()
            },
            hint: if sandbox_cert_ok {
                None
            } else {
                Some(format!(
                    "Run `pesaraja init` to download {}.",
                    sandbox_cert_path.display()
                ))
            },
        });

        checks.push(CheckItem {
            ok: false,
            critical: false,
            message: "Production credentials not configured".into(),
            hint: Some("Run `pesaraja init --env production` to set up.".into()),
        });
    } else if is_production {
        checks.push(CheckItem {
            ok: true,
            critical: false,
            message: "Production environment configured (dsk_live_)".into(),
            hint: None,
        });

        let production_cert_path = certificate_path(Environment::Production)?;
        let cert_ok = production_cert_path.exists();
        checks.push(CheckItem {
            ok: cert_ok,
            critical: false,
            message: if cert_ok {
                "Production API certificate found".into()
            } else {
                "Production API certificate not found".into()
            },
            hint: if cert_ok {
                None
            } else {
                Some(format!(
                    "Download from the Daraja portal and save to {} (or set DARAJA_CERT_PATH).",
                    production_cert_path.display()
                ))
            },
        });
    }

    checks.push(CheckItem {
        ok: true,
        critical: false,
        message: format!("Default callback URL: {DEFAULT_CALLBACK} (sandbox placeholder)"),
        hint: Some(
            "For production, expose a public HTTPS endpoint (e.g. ngrok) and pass --callback-url."
                .into(),
        ),
    });

    report(&checks, json)?;

    if has_critical_failure(&checks) {
        return Err(DarajaError::Config(
            "critical checks failed — review the report above".into(),
        ));
    }

    Ok(())
}

/// True when any check marked critical did not pass.
fn has_critical_failure(checks: &[CheckItem]) -> bool {
    checks.iter().any(|c| c.critical && !c.ok)
}

/// Render the report, as JSON for scripts or as a list for a person.
fn report(checks: &[CheckItem], json: bool) -> Result<(), DarajaError> {
    if json {
        let output = serde_json::json!({
            "critical_failed": has_critical_failure(checks),
            "checks": checks.iter().map(|check| serde_json::json!({
                "ok": check.ok,
                "message": check.message,
                "hint": check.hint,
            })).collect::<Vec<_>>(),
        });
        output::print_json(&output)?;
    } else {
        println!();
        for check in checks {
            print_check(check);
        }
        println!();
    }
    Ok(())
}

/// Checks covering the two secrets that live on disk: the encrypted
/// credentials file and the SDK key, including whether either is readable
/// by other users on the machine.
fn stored_secret_checks(
    creds_path: &std::path::Path,
    creds_exist: bool,
    key_set: bool,
    key_from_env: bool,
) -> Result<Vec<CheckItem>, DarajaError> {
    let mut checks: Vec<CheckItem> = Vec::new();

    checks.push(CheckItem {
        ok: creds_exist,
        critical: true,
        message: if creds_exist {
            format!("Credentials encrypted at rest ({})", creds_path.display())
        } else {
            "Credentials file not found".to_string()
        },
        hint: if creds_exist {
            None
        } else {
            Some("Run `pesaraja init` to set up.".to_string())
        },
    });

    if creds_exist {
        let permissions_ok = profiles::ensure_private_permissions(creds_path).is_ok();
        checks.push(CheckItem {
            ok: permissions_ok,
            critical: false,
            message: if permissions_ok {
                "Credentials file permissions are private".into()
            } else {
                "Credentials file is accessible to other users".into()
            },
            hint: if permissions_ok {
                None
            } else {
                Some(format!("Run `chmod 600 {}`.", creds_path.display()))
            },
        });
    }

    checks.push(CheckItem {
        ok: key_set,
        critical: true,
        message: if key_set {
            if key_from_env {
                "DARAJA_KEY is set (env var)".to_string()
            } else {
                format!("SDK key loaded from {}", key_path()?.display())
            }
        } else {
            "DARAJA_KEY is not set".to_string()
        },
        hint: if key_set {
            None
        } else {
            Some("Run `pesaraja init` to set up.".to_string())
        },
    });

    if !key_from_env {
        let path = key_path()?;
        if path.exists() {
            let permissions_ok = profiles::ensure_private_permissions(&path).is_ok();
            checks.push(CheckItem {
                ok: permissions_ok,
                critical: false,
                message: if permissions_ok {
                    "SDK key file permissions are private".into()
                } else {
                    "SDK key file is accessible to other users".into()
                },
                hint: if permissions_ok {
                    None
                } else {
                    Some(format!("Run `chmod 600 {}`.", path.display()))
                },
            });
        }
    }
    Ok(checks)
}
