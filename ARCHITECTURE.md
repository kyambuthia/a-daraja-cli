# Architecture

## Principles

1. **One key, one import** — developer goes zero-to-payment in 30 seconds
2. **Same key, every language** — `DARAJA_KEY` works everywhere: Rust, TS, Python, PHP, Dart
3. **No plaintext credentials on disk, ever** — if attacker steals the laptop, finances are safe
4. **Idempotency** — double-sends are impossible
5. **Callback verification** — you only ship goods when M-Pesa says "paid"
6. **Good errors** — no `500 Internal Server Error`, just sentences

## Install experience

Every target follows the same path. Two steps to a payment:

**Step 1: `mpesa init` (do this once)**

```bash
$ curl -fsSL https://get.pesaraja.dev | bash   # install the CLI
$ mpesa init                                  # enter your Daraja credentials once
  → Generates encrypted ~/.config/pesaraja/credentials.enc
  → Prints your SDK key: dsk_test_WEFWEFpj23n...==
```

**Step 2: import and use (any language, same key)**

| Language | Install | Import |
|----------|---------|--------|
| **Rust** | `cargo add pesaraja` | `use pesaraja::Daraja;` |
| **TypeScript** | `npm install @pesaraja/sdk` | `import { Daraja } from "@pesaraja/sdk"` |
| **Python** | `pip install pesaraja` | `from pesaraja import Daraja` |
| **PHP** | `composer require pesaraja/pesaraja` | `use Pesaraja\\Daraja;` |
| **Dart** | `dart pub add pesaraja` | `import 'package:pesaraja/daraja.dart';` |

Then in your `.env`:

```
DARAJA_KEY=dsk_test_WEFWEFpj23nkj23bJgFw==
```

```rust
// Rust
let d = Pesaraja::from_env()?;
d.stk_push(phone: "254712345678", amount: 500, description: "Moringa seedlings").await?;
```

```typescript
// TypeScript  
const d = Pesaraja.fromEnv();
await d.stkPush({ phone: "254712345678", amount: 500, description: "Moringa seedlings" });
```

```python
# Python
d = Pesaraja.from_env()
d.stk_push(phone="254712345678", amount=500, description="Moringa seedlings")
```

```php
// PHP
$d = Pesaraja::fromEnv();
$d->stkPush(phone: "254712345678", amount: 500, description: "Moringa seedlings");
```

```dart
// Dart
final d = Pesaraja.fromEnv();
await d.stkPush(phone: "254712345678", amount: 500, description: "Moringa seedlings");
```

Same key, same API shape, same credentials file. The CLI bootstraps everything. Every SDK reads the same encrypted blob. Porting between languages is mechanical — the design work is done.

## Why six packages (and which to ship first)

| Package | Priority | Market |
|---------|----------|--------|
| `cli` | **first** | Every dev. Universal entry point. `mpesa init`, `mpesa listen`, `mpesa doctor`. |
| `daraja-ts` | **second** | 90% of Kenyan web devs. Node/Next.js/Express. This is where adoption happens. |
| `daraja-php` | **third** | Laravel shops. `composer require`. Bigger than Rust + Python combined. |
| `daraja-py` | fourth | Django/Flask. Real but smaller. |
| `daraja-dart` | fifth | Flutter mobile startups. Forward-looking. |
| `daraja-rs` | sixth | Niche. The original. Portfolio piece. |

Monorepo layout:

```
pesaraja/
├── cli/              # Rust binary — universal entry point
├── daraja-rs/        # Rust library crate
├── daraja-ts/        # npm: @pesaraja/sdk
├── daraja-php/       # composer: pesaraja/pesaraja
├── daraja-py/        # pip: pesaraja
├── daraja-dart/      # pub.dev: pesaraja
├── ARCHITECTURE.md
├── API.md
└── README.md
```

Each SDK package is thin — typed HTTP calls + OAuth + the same encrypted config format. Not inventing, porting.

## How the key works

`dsk_test_aGVsbG8gd29ybGQ...` is NOT a Daraja consumer key. It's a 256-bit AES key, base64-encoded, with an env prefix.

During `mpesa init`:
1. User enters Daraja credentials (consumer key, secret, passkey, shortcode, etc.)
2. A random 256-bit AES key is generated — this becomes the SDK key
3. All Daraja credentials are serialized to bytes
4. Bytes are encrypted with AES-256-GCM using the SDK key
5. Encrypted blob written to `~/.config/daraja/credentials.enc`
6. SDK key returned to user: `dsk_test_<base64(AES key)>`

When the library loads:
1. Read `DARAJA_KEY` from env — this IS the AES decryption key
2. Read `~/.config/daraja/credentials.enc`
3. Decrypt with AES-256-GCM → get all Daraja credentials
4. Authenticate, make requests

**Security property:** The attacker needs BOTH the env var AND the file. One without the other is useless. AES-256-GCM provides authenticated encryption — tampering is detected.

```
┌──────────────┐     ┌──────────────────┐
│ DARAJA_KEY   │────▶│ AES-256-GCM      │
│ (256-bit)    │     │ decrypt          │
└──────────────┘     └──────┬───────────┘
                            │
               ┌────────────▼───────────┐
               │ credentials.enc        │
               │ (at rest, encrypted)   │
               └────────────┬───────────┘
                            │
               ┌────────────▼───────────┐
               │ Plaintext credentials  │
               │ (in memory only,       │
               │  zeroized on drop)     │
               └────────────────────────┘
```

## Memory safety

All credential structs implement `Drop` with `zeroize`:

```rust
struct Credentials {
    consumer_key: Zeroizing<String>,
    consumer_secret: Zeroizing<String>,
    passkey: Zeroizing<String>,
    initiator_password: Zeroizing<String>,
    // ...
}

impl Drop for Credentials {
    fn drop(&mut self) {
        // zeroize ensures the memory is wiped, not just freed
    }
}
```

No credential is ever logged, even at `trace` level. The `Debug` impl for `Credentials` prints `"<redacted>"`.

## The init wizard

```
$ mpesa init

  █▀▄ ▄▀█ █▀█ ▄▀█ ░░█ ▄▀█   █▀ █▀▄ █▄▀
  █▄▀ █▀█ █▀▄ █▀█ █▄█ █▀█   █▄ █▀▄ █░█

Setting up your M-Pesa API key.

? Environment: sandbox
? Consumer Key: ********************
? Consumer Secret: ********************
? Passkey: ********************
? Shortcode: 174379

For B2C / transaction status / reversal (optional):
? Initiator Name: apitest
? Initiator Password: ********************

✓ Encrypted credentials saved.

Your SDK key:

  dsk_test_WEFWEFpj23nkj23bJgFw==   ← THIS IS THE ONLY THING YOU NEED TO SAVE

Add to your .env:
  DARAJA_KEY="dsk_test_WEFWEFpj23nkj23bJgFw=="

Add to your production .env later with:
  mpesa init --env production
```

The user only needs to keep track of one string. Everything else is encrypted on disk.

## Idempotency: no double-sends

Every outgoing payment request includes an `Idempotency-Key` header (UUID v4). The library:

1. Generates the key before sending
2. Stores (idempotency_key, payment_ref, timestamp) in a local SQLite cache at `~/.config/daraja/idempotency.db`
3. If a request with the same key is retried, the library checks the cache:
   - If the previous attempt succeeded → return the cached result, don't re-send
   - If it failed → re-send (it was a network error, not a double-charge)
   - If it's pending → error: "a payment is already in flight with this key"

The idempotency cache auto-expires after 24 hours.

```rust
// same key, same amount, same phone → safe to retry
d.stk_push(request).await?;          // generates idempotency key internally
d.stk_push(request).await?;          // deduplicated by cache → no double-send
```

## Callback verification

For C2B callbacks, the library provides verification:

```rust
// In your web server's callback handler:
fn handle_c2b_callback(req: HttpRequest) -> Result<()> {
    let callback = daraja::verify_callback(
        &req.body(),
        req.header("X-Safaricom-Signature"),
    )?;
    // callback is verified: it came from Safaricom, not a spoofed request
    // Only now do you ship the goods
}
```

The `mpesa listen` command implements this automatically for local testing.

## `mpesa listen` — the killer dev tool

```
$ mpesa listen
✓ Reading credentials for sandbox
✓ Starting local server on http://localhost:8181
✓ Registering C2B URLs with Daraja sandbox
✗ Your server must be reachable from the internet for production.
  In sandbox, C2B simulate will work locally.

Waiting for callbacks...

← C2B Confirmation
  Transaction:  QEJ78H8JKR
  Phone:        254712345678
  Amount:       KES 500
  Reference:    Test payment
  Time:         2026-06-10 10:23:45
  ✓ Signature verified
```

Forwards webhooks to a local endpoint, prints them nicely, verifies signatures. Stops the "push to production to test a callback" loop.

## Shared config format

All six packages read the same encrypted file. The format is documented, not guessed.

```
~/.config/daraja/
├── credentials.enc       # AES-256-GCM encrypted credential blob
├── credentials.json      # Plaintext structure (for reference, never written by init)
│                         # {
│                         #   "sandbox": { consumer_key, consumer_secret, passkey,
│                         #                 shortcode, initiator_name, initiator_password },
│                         #   "production": { ... }
│                         # }
└── idempotency.db        # SQLite cache for idempotency keys
```

The encrypted blob contains JSON with both sandbox and production credentials. The `dsk_test_` and `dsk_live_` prefixes tell the SDK which environment to use. The key itself unlocks the whole file — both environments are inside.

This means:
- `mpesa init --env sandbox && mpesa init --env production` — both stored in the same blob
- Switching env is just changing the key prefix: `dsk_test_xxx` → `dsk_live_xxx` (different key, different blob section)
- Actually: same AES key, same encrypted blob, the prefix tells the SDK which section to read

## Reference Rust crate structure

```
daraja-rs/               # library — what devs import
  src/
    lib.rs                # Daraja struct, from_env(), from_key()
    client.rs             # reqwest wrapper, retry, idempotency
    auth.rs               # OAuth token lifecycle
    api/
      stk_push.rs         # one fn per endpoint
      b2c.rs
      c2b.rs
      status.rs
      balance.rs
      reversal.rs
    security.rs           # SecurityCredential RSA encryption
    config.rs             # encrypted config load/save
    webhook.rs            # callback verification
    types.rs              # request/response structs
    error.rs              # Error enum with human messages

cli/                      # thin binary — calls daraja-rs
  src/
    main.rs               # clap dispatch
    init.rs               # setup wizard (dialoguer)
    listen.rs             # C2B webhook forwarder
    doctor.rs             # production readiness check
```

Other language packages mirror this structure with their idiomatic equivalents (npm workspace, setuptools, composer, pubspec).

## Error design

Every error maps to a sentence. Never a raw status code.

```
× Authentication failed: invalid consumer key.
× The phone number '0712345678' is not registered on M-Pesa.
× STK push cancelled: user declined the payment.
× STK push timed out: user did not enter PIN within 60 seconds.
× Insufficient funds in your M-Pesa account (balance: KES 12).
× Rate limited: too many requests. Wait 5 seconds.
× Daraja API returned an unknown error: '999'. Please report this.
```

Error messages are in English (the universal dev language). Swahili variants possible via feature flag in the future.

## Rate limiting

The library respects Daraja rate limits:
- STK push: max 1 per 2 seconds per phone number
- Transaction status: max 10 per minute
- B2C: max 10 per minute

Built-in backpressure. The library queues and spaces out requests automatically.

## Production readiness checklist

```
$ mpesa doctor

✓ Credentials encrypted at rest
✓ DARAJA_KEY is set
✓ Sandbox OAuth token: valid (expires in 3489s)
✓ STK Push: working (tested with KES 1)
✗ B2C: not available in sandbox (needs Go Live)
✓ Callback URL: reachable
✓ SSL on callback URL: valid
✗ Production credentials not configured
  Run `mpesa init --env production` to set up.
```

One command tells the developer what's missing before they go live.
