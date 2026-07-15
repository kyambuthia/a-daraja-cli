use crate::{load_daraja, load_sdk_key, load_shortcode};
use daraja_rs::api::c2b::register_urls;
use daraja_rs::error::DarajaError;
use daraja_rs::webhook::{C2bConfirmation, validate_c2b_confirmation};
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::convert::Infallible;
use std::net::SocketAddr;
use tokio::net::TcpListener;

pub async fn run(port: u16) -> Result<(), DarajaError> {
    let sdk_key = load_sdk_key()?;
    let is_sandbox = sdk_key.starts_with("dsk_test_");
    let env_label = if is_sandbox { "sandbox" } else { "production" };

    if !is_sandbox {
        return Err(DarajaError::Config(
            "pesaraja receive is sandbox-only until cryptographic callback verification is configured".into(),
        ));
    }

    println!("✓ Reading credentials for {env_label}");

    let daraja = load_daraja().await?;
    let shortcode = load_shortcode()?;

    let confirm_url = format!("http://localhost:{port}/c2b/confirm");
    let validation_url = format!("http://localhost:{port}/c2b/validate");

    println!("✓ Starting local server on http://localhost:{port}");

    if is_sandbox {
        println!("✓ Registering C2B URLs with Daraja sandbox");
    } else {
        println!("✗ Your server must be reachable from the internet for production.");
        println!("  Register public HTTPS URLs with `pesaraja register` before going live.");
    }

    register_urls(
        daraja.client(),
        &shortcode,
        "Completed",
        &confirm_url,
        &validation_url,
    )
    .await?;

    if is_sandbox {
        println!("✗ Your server must be reachable from the internet for production.");
        println!("  In sandbox, run `pesaraja simulate` to deliver test payments here.");
    }

    println!();
    println!("Waiting for incoming payments...");
    println!();

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| DarajaError::Config(format!("failed to bind to port {port}: {e}")))?;

    loop {
        let (stream, _) = listener
            .accept()
            .await
            .map_err(|e| DarajaError::Config(format!("failed to accept connection: {e}")))?;
        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let service = service_fn(handle_request);
            if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                eprintln!("  connection error: {err}");
            }
        });
    }
}

async fn handle_request(
    req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let path = req.uri().path().to_string();
    let method = req.method().clone();

    if method != Method::POST {
        return Ok(not_found());
    }

    match path.as_str() {
        "/c2b/confirm" => handle_c2b_callback(req, "Confirmation").await,
        "/c2b/validate" => handle_c2b_callback(req, "Validation").await,
        _ => Ok(not_found()),
    }
}

async fn handle_c2b_callback(
    req: Request<hyper::body::Incoming>,
    kind: &str,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let body = match req.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(err) => {
            eprintln!("  × Failed to read request body: {err}");
            return Ok(error_response());
        }
    };

    let confirmation: C2bConfirmation = match validate_c2b_confirmation(&body) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("  × Invalid C2B {kind} payload: {err}");
            return Ok(error_response());
        }
    };

    print_callback(kind, &confirmation);

    Ok(accepted_response())
}

fn print_callback(kind: &str, conf: &C2bConfirmation) {
    println!("← C2B {kind}");
    println!("  Transaction:  {}", conf.trans_id);
    println!("  Phone:        {}", conf.msisdn);
    println!("  Amount:       {}", format_amount(&conf.trans_amount));
    println!("  Reference:    {}", conf.bill_ref_number);
    println!("  Time:         {}", format_trans_time(&conf.trans_time));
    println!("  ⚠ Sandbox callback accepted for local testing only");
    println!("  ⚠ Cryptographic callback verification is not configured");
    println!();
}

fn format_amount(amount: &str) -> String {
    let amount = amount.trim();
    if let Some((whole, frac)) = amount.split_once('.')
        && frac.chars().all(|c| c == '0')
    {
        return format!("KES {whole}");
    }
    format!("KES {amount}")
}

fn format_trans_time(raw: &str) -> String {
    if raw.len() == 14 && raw.chars().all(|c| c.is_ascii_digit()) {
        format!(
            "{}-{}-{} {}:{}:{}",
            &raw[0..4],
            &raw[4..6],
            &raw[6..8],
            &raw[8..10],
            &raw[10..12],
            &raw[12..14]
        )
    } else {
        raw.to_string()
    }
}

fn accepted_response() -> Response<Full<Bytes>> {
    let body = r#"{"ResultCode":0,"ResultDesc":"Accepted"}"#;
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
}

fn error_response() -> Response<Full<Bytes>> {
    let body = r#"{"ResultCode":1,"ResultDesc":"Rejected"}"#;
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
}

fn not_found() -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Full::new(Bytes::new()))
        .unwrap()
}
