# Usage

## Common CLI Options

- `--json`: machine-readable output for inspection and response commands that support it.
- `--dry-run`: print the request summary without calling Daraja on money-moving commands that support it.
- `-y`, `--yes`: skip confirmation prompts on supported commands.

Amounts must be positive whole KES. The CLI accepts an all-zero decimal suffix
such as `100.00`, but rejects non-zero fractions such as `100.50`.

## Collect Money With STK Push

Sandbox dry run:

```bash
pesaraja prompt \
  --phone 254712345678 \
  --amount 500 \
  --description "Order payment" \
  --reference "ORDER-123" \
  --dry-run
```

Send a real prompt:

```bash
pesaraja prompt \
  --phone 254712345678 \
  --amount 500 \
  --description "Order payment" \
  --reference "ORDER-123" \
  --callback-url https://example.com/api/mpesa/callback
```

Use Till / Buy Goods transaction type:

```bash
pesaraja prompt \
  --phone 254712345678 \
  --amount 500 \
  --type till \
  --callback-url https://example.com/api/mpesa/callback
```

Production requires `--callback-url`. Sandbox falls back to a placeholder callback URL if omitted, but that is only useful for basic request testing. All callback URLs are validated even during `--dry-run`.

Check prompt status:

```bash
pesaraja prompt-status --checkout-id ws_CO_...
pesaraja --json prompt-status --checkout-id ws_CO_...
```

Inspect locally persisted payment state:

```bash
pesaraja payments list
pesaraja payments inspect --id pay_...
pesaraja payments reconcile --id pay_...
```

If a payment is definitively failed, preview a safe retry plan without sending
anything:

```bash
pesaraja payments retry --id pay_... --dry-run
```

## Receive And Simulate C2B Payments

Start the local callback server:

```bash
pesaraja receive --port 8181
```

Use this for sandbox/local development. Production callbacks need public HTTPS URLs.

Simulate a sandbox C2B payment:

```bash
pesaraja simulate \
  --phone 254708374149 \
  --amount 100 \
  --reference "ORDER-123"
```

For deterministic local integration testing, run the built-in Daraja simulator:

```bash
pesaraja simulator --scenario success
```

It binds to `127.0.0.1:8787` by default and models OAuth, STK initiation, and
STK query responses. Use `--scenario pending`, `failed`, `malformed`, or
`provider-drift` to exercise ambiguous, rejected, malformed, or provider-shape
responses without contacting Safaricom.

Point a sandbox prompt at the local simulator with the loopback-only endpoint
override:

```bash
pesaraja prompt \
  --phone 254712345678 \
  --amount 500 \
  --reference "ORDER-123" \
  --callback-url http://127.0.0.1:8181/callback \
  --base-url http://127.0.0.1:8787
```

The same `--base-url` can be used with `pesaraja prompt-status` and
`pesaraja payments reconcile`. It accepts only an HTTP loopback host and
requires sandbox credentials; production endpoints cannot be overridden.

Register public C2B URLs:

```bash
pesaraja register \
  --confirm-url https://example.com/api/mpesa/c2b/confirm \
  --validation-url https://example.com/api/mpesa/c2b/validate
```

Send a sample callback to your local app:

```bash
pesaraja webhook http://localhost:8181/c2b confirm
```

## Send Money With B2C

Dry run:

```bash
pesaraja send \
  --to 254712345678 \
  --amount 100 \
  --remarks "Refund" \
  --dry-run
```

Production requires both `--result-url` and `--timeout-url`; the CLI never
silently sends asynchronous results to the example URLs. Sandbox uses the
example URLs only when those flags are omitted, and validates supplied URLs
before a dry run is printed.

Send:

```bash
pesaraja send \
  --to 254712345678 \
  --amount 100 \
  --remarks "Refund" \
  --result-url https://example.com/api/mpesa/result \
  --timeout-url https://example.com/api/mpesa/timeout
```

Payment types:

```bash
pesaraja send --type business ...
pesaraja send --type salary ...
pesaraja send --type promotion ...
```

B2C requires initiator credentials and the correct Safaricom certificate.

## Query, Balance, And Refund

Query transaction status:

```bash
pesaraja query \
  --id QEJ78H8JKR \
  --result-url https://example.com/api/mpesa/result \
  --timeout-url https://example.com/api/mpesa/timeout
```

Check balance:

```bash
pesaraja balance \
  --result-url https://example.com/api/mpesa/result \
  --timeout-url https://example.com/api/mpesa/timeout
```

The balance result arrives asynchronously at your result URL.

Reverse a payment:

```bash
pesaraja refund \
  --id QEJ78H8JKR \
  --amount 100 \
  --receiver 600000 \
  --receiver-type 11 \
  --result-url https://example.com/api/mpesa/result \
  --timeout-url https://example.com/api/mpesa/timeout
```

## Rust SDK

```rust
use daraja_rs::Daraja;

let daraja = Daraja::from_env()?;
let client = daraja.client();
```

The Rust SDK currently exposes lower-level API functions through `daraja_rs::api::*`. `Daraja::from_env()` requires `DARAJA_KEY`; `Daraja::from_key(...)` decrypts `credentials.enc` in the platform config directory.

Typed options are available for common requests to avoid long positional
argument lists:

```rust
use daraja_rs::api::stk_push::{stk_push_with_options, StkPushOptions};

let options = StkPushOptions::new(
    "254712345678",
    500,
    "https://example.com/callback",
    "order:ORDER-123:stk",
)
    .with_reference("ORDER-123");
let response = stk_push_with_options(daraja.client(), &options).await?;
```

## Troubleshooting

Run:

```bash
pesaraja doctor
```

Common issues:

- Missing key: run `pesaraja init`.
- Wrong key for encrypted credentials: export the key printed by `pesaraja export-key` or rerun `pesaraja init`.
- Sandbox passkey mismatch: rerun `pesaraja init`; sandbox uses the fixed public passkey.
- Missing initiator credentials: rerun `pesaraja init` and configure initiator details.
- Missing production certificate: save `production.cer` in the platform config directory or set `DARAJA_CERT_PATH`.
