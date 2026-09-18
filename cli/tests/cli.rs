use daraja_rs::config::{
    Credentials, encrypt_credentials, extract_key_from_sdk_key, generate_sdk_key,
};
use std::io::{BufRead, BufReader};
use std::net::TcpListener;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use zeroize::Zeroizing;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_pesaraja")
}

fn command(config_dir: &std::path::Path) -> Command {
    let mut command = Command::new(binary());
    command
        .env("DARAJA_CONFIG_DIR", config_dir)
        .env_remove("DARAJA_KEY")
        .env_remove("PESARAJA_PROFILE");
    command
}

#[test]
fn help_identifies_the_product_and_binary() {
    let directory = tempfile::tempdir().unwrap();
    let output = command(directory.path()).arg("--help").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("Collect payments"));
    assert!(stdout.contains("Usage: pesaraja"));
}

#[test]
fn json_errors_have_a_stable_shape_and_exit_code() {
    let directory = tempfile::tempdir().unwrap();
    let output = command(directory.path())
        .args(["--json", "config"])
        .output()
        .unwrap();
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(error["error"]["code"], "configuration_error");
    assert_eq!(error["error"]["outcomeUnknown"], false);
}

#[test]
fn simulator_drives_a_cli_stk_golden_path() {
    let directory = tempfile::tempdir().unwrap();
    let sdk_key = generate_sdk_key("test");
    let key = extract_key_from_sdk_key(&sdk_key).unwrap();
    let credentials = Credentials {
        consumer_key: Zeroizing::new("sim-key".into()),
        consumer_secret: Zeroizing::new("sim-secret".into()),
        passkey: Zeroizing::new(
            "bfb279f9aa9bdbcf158e97dd71a467cd2e0c893059b10f78e6b72ada1ed2c919".into(),
        ),
        shortcode: "174379".into(),
        initiator_name: None,
        initiator_password: None,
    };
    std::fs::write(directory.path().join("key"), &sdk_key).unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(
        directory.path().join("key"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    std::fs::write(
        directory.path().join("credentials.enc"),
        encrypt_credentials(&credentials, &key).unwrap(),
    )
    .unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(
        directory.path().join("credentials.enc"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();

    let reserved = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = reserved.local_addr().unwrap().port();
    drop(reserved);
    let mut simulator = command(directory.path())
        .args(["simulator", "--port", &port.to_string()])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = simulator.stdout.take().unwrap();
    let mut lines = BufReader::new(stdout).lines();
    let ready = lines.next().unwrap().unwrap();
    assert!(ready.contains("Local Daraja simulator listening"));
    // Keep the piped stdout reader alive in the background so the simulator
    // never sees a closed pipe while printing its remaining startup lines.
    std::thread::spawn(move || {
        for next in lines.by_ref() {
            if next.is_err() {
                break;
            }
        }
    });

    let output = command(directory.path())
        .args([
            "prompt",
            "--phone",
            "254712345678",
            "--amount",
            "100",
            "--reference",
            "E2E-1",
            "--description",
            "CLI integration test",
            "--idempotency-key",
            "test:cli:e2e:1",
            "--base-url",
            &format!("http://127.0.0.1:{port}"),
            "--yes",
        ])
        .output()
        .unwrap();

    let _ = simulator.kill();
    let _ = simulator.wait();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("sim-co-1"));

    let payments = command(directory.path())
        .args(["--json", "payments", "list"])
        .output()
        .unwrap();
    let ledger: serde_json::Value = serde_json::from_slice(&payments.stdout).unwrap();
    assert_eq!(ledger[0]["kind"], "stk");
    assert_eq!(ledger[0]["status"], "pending");
}

fn setup_sandbox_config(config_dir: &std::path::Path) {
    let sdk_key = generate_sdk_key("test");
    let key = extract_key_from_sdk_key(&sdk_key).unwrap();
    let credentials = Credentials {
        consumer_key: Zeroizing::new("sim-key".into()),
        consumer_secret: Zeroizing::new("sim-secret".into()),
        passkey: Zeroizing::new(
            "bfb279f9aa9bdbcf158e97dd71a467cd2e0c893059b10f78e6b72ada1ed2c919".into(),
        ),
        shortcode: "174379".into(),
        initiator_name: None,
        initiator_password: None,
    };
    std::fs::write(config_dir.join("key"), &sdk_key).unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(
        config_dir.join("key"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    std::fs::write(
        config_dir.join("credentials.enc"),
        encrypt_credentials(&credentials, &key).unwrap(),
    )
    .unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(
        config_dir.join("credentials.enc"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
}

struct Simulator {
    child: std::process::Child,
    base_url: String,
}

impl Simulator {
    fn start(config_dir: &std::path::Path, scenario: &str) -> Self {
        let reserved = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = reserved.local_addr().unwrap().port();
        drop(reserved);
        let mut child = command(config_dir)
            .args([
                "simulator",
                "--port",
                &port.to_string(),
                "--scenario",
                scenario,
            ])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let mut lines = BufReader::new(stdout).lines();
        let ready = lines.next().unwrap().unwrap();
        assert!(
            ready.contains("Local Daraja simulator listening"),
            "simulator ready line: {ready}"
        );
        // Drain remaining startup output in the background so the simulator
        // never blocks or panics on a closed stdout pipe while tests run.
        std::thread::spawn(move || {
            for next in lines.by_ref() {
                if next.is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            base_url: format!("http://127.0.0.1:{port}"),
        }
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }
}

impl Drop for Simulator {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn ledger_as_json(config_dir: &std::path::Path) -> serde_json::Value {
    let output = command(config_dir)
        .args(["--json", "payments", "list"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn simulator_prompt_status_success_reports_simulated_ids_and_leaves_ledger_empty() {
    let directory = tempfile::tempdir().unwrap();
    setup_sandbox_config(directory.path());
    let simulator = Simulator::start(directory.path(), "success");

    let output = command(directory.path())
        .args([
            "prompt-status",
            "--checkout-id",
            "sim-co-1",
            "--base-url",
            simulator.base_url(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Prompt status retrieved"));
    assert!(stdout.contains("sim-co-1"));
    assert!(stdout.contains("sim-mr-1"));
    assert!(stdout.contains("Request accepted"));

    assert_eq!(ledger_as_json(directory.path()), serde_json::json!([]));
}

#[test]
fn simulator_prompt_status_failed_reports_simulated_ids_and_leaves_ledger_empty() {
    let directory = tempfile::tempdir().unwrap();
    setup_sandbox_config(directory.path());
    let simulator = Simulator::start(directory.path(), "failed");

    let output = command(directory.path())
        .args([
            "--json",
            "prompt-status",
            "--checkout-id",
            "sim-co-1",
            "--base-url",
            simulator.base_url(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let body: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(body["checkout_request_id"], "sim-co-1");
    assert_eq!(body["merchant_request_id"], "sim-mr-1");
    assert_eq!(body["response_code"], "0");
    assert_eq!(body["response_description"], "Request accepted");

    assert_eq!(ledger_as_json(directory.path()), serde_json::json!([]));
}

#[test]
fn simulator_prompt_status_malformed_fails_closed_and_leaves_ledger_empty() {
    let directory = tempfile::tempdir().unwrap();
    setup_sandbox_config(directory.path());
    let simulator = Simulator::start(directory.path(), "malformed");

    let output = command(directory.path())
        .args([
            "--json",
            "prompt-status",
            "--checkout-id",
            "sim-co-1",
            "--base-url",
            simulator.base_url(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], "network_error");
    assert_eq!(error["error"]["outcomeUnknown"], false);

    assert_eq!(ledger_as_json(directory.path()), serde_json::json!([]));
}

#[test]
fn reconcile_rejects_b2c_and_reversal_records_without_credentials() {
    use daraja_rs::payments::{NewPayment, PaymentKind, PaymentStore};

    for (kind, id, idempotency_key) in [
        (PaymentKind::B2c, "b2c-1", "test:reconcile:b2c:1"),
        (PaymentKind::Reversal, "rev-1", "test:reconcile:rev:1"),
    ] {
        // Intentionally no key/credentials files: the kind guard must fire
        // before credential loading or any provider network I/O.
        let directory = tempfile::tempdir().unwrap();
        let mut store = PaymentStore::new(directory.path().join("payments.db")).unwrap();
        store
            .create(&NewPayment {
                id: id.into(),
                kind,
                environment: "sandbox".into(),
                shortcode: "600000".into(),
                amount_kes: 100,
                phone: Some("254712345678".into()),
                reference: "Test".into(),
                idempotency_key: Some(idempotency_key.into()),
            })
            .unwrap();
        drop(store);

        let output = command(directory.path())
            .args(["--json", "payments", "reconcile", "--id", id])
            .output()
            .unwrap();
        assert!(!output.status.success(), "kind: {}", kind.as_str());
        assert_eq!(output.status.code(), Some(2), "kind: {}", kind.as_str());
        let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["code"], "configuration_error");
        assert_eq!(error["error"]["outcomeUnknown"], false);
        let message = error["error"]["message"].as_str().unwrap().to_lowercase();
        assert!(
            message.contains(kind.as_str()),
            "error names the record kind ({kind:?}): {message}"
        );
        assert!(
            message.contains("asynchronous results cannot be authoritatively reconciled"),
            "error states the async boundary: {message}"
        );

        // Unsupported kinds leave local state untouched.
        let ledger = ledger_as_json(directory.path());
        assert_eq!(ledger[0]["id"], id);
        assert_eq!(ledger[0]["kind"], kind.as_str());
    }
}
