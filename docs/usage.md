# Usage

## Common CLI Options

- `--json`: machine-readable output for inspection and response commands that support it.
- `--dry-run`: print the request summary without calling Daraja on money-moving commands that support it.
- `-y`, `--yes`: skip confirmation prompts on supported commands.

Amounts are whole KES. Decimal input is truncated and warns when the decimal portion is non-zero.

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

Production requires `--callback-url`. Sandbox falls back to a placeholder callback URL if omitted, but that is only useful for basic request testing.

Check prompt status:

```bash
pesaraja prompt-status --checkout-id ws_CO_...
pesaraja --json prompt-status --checkout-id ws_CO_...
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

## TypeScript SDK

```typescript
import { Daraja, parseStkCallback, stkCallbackPaid } from "@pesaraja/sdk";

const daraja = await Daraja.fromEnv();

const response = await daraja.prompt({
  phone: "254712345678",
  amount: 500,
  callbackUrl: "https://example.com/api/mpesa/callback",
  reference: "ORDER-123",
  description: "Order payment",
});

console.log(response.CheckoutRequestID);
```

`Daraja.fromEnv()` reads `DARAJA_KEY` first, then falls back to `~/.config/daraja/key`. The SDK decrypts `~/.config/daraja/credentials.enc`.

STK callback example:

```typescript
const callback = parseStkCallback(requestBody);
if (stkCallbackPaid(callback)) {
  // Mark the order paid using callback.CheckoutRequestID and metadata.
}
```

C2B callback helpers:

```typescript
import { parseC2bConfirmation, verifyCallback } from "@pesaraja/sdk";

verifyCallback(rawBody, signatureHeader);
const confirmation = parseC2bConfirmation(JSON.parse(rawBody));
```

Current limitation: callback cryptographic verification is not configured. The SDK
fails closed when `verifyCallback` is called; do not fulfill orders from an
unverified callback. The CLI `receive` command is sandbox-only and labels local
callbacks as unverified.

## Rust SDK

```rust
use daraja_rs::Daraja;

let daraja = Daraja::from_env()?;
let client = daraja.client();
```

The Rust SDK currently exposes lower-level API functions through `daraja_rs::api::*`. `Daraja::from_env()` requires `DARAJA_KEY`; `Daraja::from_key(...)` decrypts `~/.config/daraja/credentials.enc`.

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
- Missing production certificate: save `production.cer` under `~/.config/daraja/` or set `DARAJA_CERT_PATH`.
