# Pesaraja Architecture

This document describes the implementation in this repository. Future product
ideas belong in [`roadmap.md`](../roadmap.md) and must not be treated as shipped
guarantees.

## Packages

| Package | Responsibility |
| --- | --- |
| `daraja-rs` | Rust Daraja client, request types, credentials, certificates, validation, idempotency, and callback parsing. |
| `cli` | The `pesaraja` command-line interface and local sandbox callback receiver. |
| `daraja-ts` | The ESM `@pesaraja/sdk` TypeScript client for Node.js 18 and newer. |

The Rust and TypeScript SDKs intentionally share the same Daraja field names,
credential blob format, validation contract, and environment prefixes.

## Request lifecycle

Mutating API requests follow this sequence:

1. Validate amount, phone, identifiers, text fields, URLs, and environment.
2. Acquire a locally persisted, operation-scoped idempotency reservation.
3. Obtain or reuse an OAuth token with a 30-second expiry safety window.
4. Send one HTTP request with a bounded deadline.
5. Refresh authentication once after a definitive `401` response.
6. Persist the parsed response, definitive failure, or unknown outcome.

OAuth acquisition may retry bounded transient failures. Money-moving POSTs are
not automatically retried after network failures because Daraja may already
have received the request.

Status, balance, and STK query POSTs bypass local idempotency caching and the
`Idempotency-Key` header so repeated queries receive fresh results.

## Idempotency

Callers should provide a stable business-operation key:

```text
order:ORDER-123:stk
disbursement:PAYROLL-2026-07:EMPLOYEE-42
reversal:QEJ78H8JKR
```

The local persistence key is scoped by environment, shortcode, API endpoint,
and caller key. Reusing a key with a different serialized request is rejected.
Concurrent acquisition is atomic in Rust's SQLite store and protected by an
atomic file/lock protocol in TypeScript.

An `unknown` state means a request may have reached Daraja but no response was
obtained. Applications must reconcile that transaction before using a new key.

This is local duplicate protection, not a distributed exactly-once guarantee.
Multiple hosts must use a shared application-level idempotency store or route a
business operation through one coordinator.

Rust applications can provide an `IdempotencyBackend` through
`with_idempotency_backend`; TypeScript applications can provide an
`IdempotencyBackend` in `DarajaClientOptions`. SQLite and the JSON file store
remain local defaults, not distributed coordination.

## Credentials

`pesaraja init` stores files under `~/.config/daraja/`:

```text
key                       local AES key, mode 0600 on Unix
credentials.enc           AES-256-GCM credential blob
profiles.json             non-secret profile metadata
profiles/personal.enc     encrypted profile credentials
idempotency.db            Rust idempotency state
idempotency-ts.json        TypeScript idempotency state
```

The `dsk_test_` and `dsk_live_` prefixes select sandbox and production. The
suffix is the AES key used to decrypt the credential blob.

Because the local key and encrypted blob are normally stored under the same OS
account, this design protects against accidental disclosure and access by other
users with correctly enforced permissions. It does not protect credentials
after compromise of the same user account. Production deployments should pass
the SDK key through a secret manager and may construct the TypeScript client
directly with `Daraja.fromCredentials(...)`.

## Callback trust model

Callback payloads are untrusted input. The SDKs provide:

- Structural and domain validation.
- Matching of STK checkout ID, merchant request ID, amount, and receipt.
- Stable deduplication keys for replay prevention.
- `HmacSha256Authenticator` for an application-owned callback gateway that
  signs the raw body with a shared secret.
- `CallbackReplayStore` for an atomic, seven-day SQLite replay claim.
- A fail-closed cryptographic verification placeholder.

Applications that have an independently documented authenticator can supply it
through `verify_callback_with` (Rust) or `verifyCallbackWith` (TypeScript).
This extension point does not define or imply a Safaricom signature algorithm;
callbacks still require replay protection and authoritative reconciliation before
fulfilment.

The public callback contract available to this project does not provide a
cryptographic verification algorithm and key-management scheme that can be
implemented safely. Header presence is never authentication. A callback that
matches an initiated payment is still not cryptographically trusted.

Before irreversible fulfilment, applications should reconcile payment state
through their authoritative Daraja transaction flow and atomically record the
receipt or transaction ID so replay cannot fulfil twice.

The HMAC helper uses the application-owned header format
`sha256=<hex-digest>`. It does not authenticate a direct Safaricom callback and
must not be described as a Safaricom signature verifier. Applications using a
gateway can verify the raw body, validate the parsed callback, then claim
`c2b_callback_deduplication_key` in `CallbackReplayStore` before fulfilment.

The CLI `receive` server is therefore sandbox-only.

## Environment boundaries

- Sandbox and production have separate base URLs and SDK key prefixes.
- Production callback, result, and timeout URLs must use HTTPS.
- C2B simulation and the local callback receiver are rejected in production.
- Production certificate configuration never falls back to a sandbox certificate.

## Error handling

HTTP requests use connection and total-request deadlines. API failures expose a
bounded status, code, and message selected from known Daraja fields. Raw
response bodies are omitted so unexpected fields cannot leak credentials or
full payment payloads through logs.

## Testing

Tests use temporary directories, fixtures, generated certificates, and
`wiremock`/mocked `fetch`. They never require real Daraja credentials or send
live transactions. The shared
[validation contract](../daraja-rs/fixtures/validation-contract.json) is
consumed by both SDK test suites to keep boundary behavior aligned.
