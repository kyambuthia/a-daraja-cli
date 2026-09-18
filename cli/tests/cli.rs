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
