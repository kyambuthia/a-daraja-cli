//! Local Daraja-compatible HTTP simulator for deterministic integration tests.

use daraja_rs::error::DarajaError;
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::convert::Infallible;
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum Scenario {
    /// Return successful STK initiation and query responses.
    Success,
    /// Return an accepted query without a final result code.
    Pending,
    /// Return a failed STK query result.
    Failed,
    /// Return a deliberately incomplete STK response.
    Malformed,
    /// Return the observed sandbox C2B response-shape drift.
    ProviderDrift,
}

impl Scenario {
    fn label(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Pending => "pending",
            Self::Failed => "failed",
            Self::Malformed => "malformed",
            Self::ProviderDrift => "provider-drift",
        }
    }
}

pub async fn run(port: u16, scenario: Scenario) -> Result<(), DarajaError> {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(address)
        .await
        .map_err(|error| DarajaError::Config(format!("bind simulator on {address}: {error}")))?;
    let address = listener
        .local_addr()
        .map_err(|error| DarajaError::Config(format!("read simulator address: {error}")))?;

    println!("✓ Local Daraja simulator listening on http://{address}");
    println!("✓ Scenario: {}", scenario.label());
    println!("  OAuth: GET /oauth/v1/generate");
    println!("  STK:  POST /mpesa/stkpush/v1/processrequest");
    println!("  Query: POST /mpesa/stkpush/v1/query");
    println!("  Press Ctrl-C to stop.");

    loop {
        tokio::select! {
            result = listener.accept() => {
                let (stream, _) = result
                    .map_err(|error| DarajaError::Network(format!("accept simulator connection: {error}")))?;
                tokio::spawn(async move {
                    let io = TokioIo::new(stream);
                    let service = service_fn(move |request| handle(request, scenario));
                    if let Err(error) = http1::Builder::new().serve_connection(io, service).await {
                        eprintln!("simulator connection error: {error}");
                    }
                });
            }
            result = tokio::signal::ctrl_c() => {
                result.map_err(|error| DarajaError::Network(format!("wait for simulator shutdown: {error}")))?;
                println!("\n✓ Simulator stopped");
                return Ok(());
            }
        }
    }
}

async fn handle(
    request: Request<hyper::body::Incoming>,
    scenario: Scenario,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let _ = request.collect().await;

    let response = match (method, path.as_str()) {
        (Method::GET, "/oauth/v1/generate") => json(
            StatusCode::OK,
            r#"{"access_token":"simulator-token","expires_in":"3599"}"#,
        ),
        (Method::POST, "/mpesa/stkpush/v1/processrequest") => match scenario {
            Scenario::Malformed => json(StatusCode::OK, r#"{"ResponseCode":"0"}"#),
            _ => json(
                StatusCode::OK,
                r#"{"MerchantRequestID":"sim-mr-1","CheckoutRequestID":"sim-co-1","ResponseCode":"0","ResponseDescription":"Success","CustomerMessage":"Success"}"#,
            ),
        },
        (Method::POST, "/mpesa/stkpush/v1/query") => match scenario {
            Scenario::Success => json(
                StatusCode::OK,
                r#"{"MerchantRequestID":"sim-mr-1","CheckoutRequestID":"sim-co-1","ResponseCode":"0","ResponseDescription":"Request accepted","ResultCode":"0","ResultDesc":"The service request is processed successfully."}"#,
            ),
            Scenario::Failed => json(
                StatusCode::OK,
                r#"{"MerchantRequestID":"sim-mr-1","CheckoutRequestID":"sim-co-1","ResponseCode":"0","ResponseDescription":"Request accepted","ResultCode":"1032","ResultDesc":"Request cancelled by user."}"#,
            ),
            Scenario::Malformed => json(StatusCode::OK, r#"{"ResponseCode":"0"}"#),
            _ => json(
                StatusCode::OK,
                r#"{"MerchantRequestID":"sim-mr-1","CheckoutRequestID":"sim-co-1","ResponseCode":"0","ResponseDescription":"Request accepted"}"#,
            ),
        },
        (Method::POST, "/mpesa/c2b/v1/simulate") if matches!(scenario, Scenario::ProviderDrift) => {
            json(
                StatusCode::OK,
                r#"{"ConversationID":"sim-c2b-1","ResponseDescription":"Success"}"#,
            )
        }
        (Method::POST, _) => json(
            StatusCode::NOT_FOUND,
            r#"{"errorCode":"SIMULATOR_NOT_FOUND","errorMessage":"endpoint not modeled"}"#,
        ),
        _ => json(
            StatusCode::METHOD_NOT_ALLOWED,
            r#"{"errorCode":"SIMULATOR_METHOD_NOT_ALLOWED"}"#,
        ),
    };

    Ok(response)
}

fn json(status: StatusCode, body: &str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(body.to_owned())))
        .expect("simulator response is valid")
}
