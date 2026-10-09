# A Daraja CLI

> **Alpha software:** do not use for production money movement yet. Callback
> signature verification is not currently implemented.

> A safer command-line interface and Rust SDK for Safaricom Daraja and M-Pesa.

**Naming:** A Daraja CLI is the project and repository. `pesaraja` is the
installed command and `daraja-rs` is the Rust crate. Both names remain stable
for existing scripts.

Current code-backed docs:

- [Install](docs/install.md)
- [Setup](docs/setup.md)
- [Usage](docs/usage.md)
- [API reference](API.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Release process](docs/releasing.md)
- [Changelog](CHANGELOG.md)

Safaricom's [Daraja API](https://developer.safaricom.co.ke) is a bridge. A broken one. This is the money bridge.

## The problem

To send a single M-Pesa payment via Daraja you need OAuth tokens, passkeys, base64 passwords, timestamps, environment-specific URLs, and headers. This project handles all of that.

## The fix

```bash
# Collect payment from customer (STK Push)
pesaraja prompt --phone 254712345678 --amount 500

# Send money to a phone (B2C — production requires Go Live)
pesaraja send --to 254712345678 --amount 100

# Query transaction / balance / refund
pesaraja query --id QEJ78H8JKR
pesaraja balance
pesaraja refund --id QEJ78H8JKR --amount 100
```

## Install

```bash
git clone https://github.com/kyambuthia/a-daraja-cli
cd a-daraja-cli
cargo build --release -p cli
cp target/release/pesaraja ~/.local/bin/
```

Or use `./install.sh` with `PESARAJA_VERSION` when a GitHub release is
available. Prebuilt binaries must pass the release SHA-256 checksum; otherwise
the installer builds the selected tag from source.

## Setup

```bash
pesaraja init
```

Walks you through Daraja credentials, encrypts them in the platform config
directory, and prints your SDK key. Set `DARAJA_CONFIG_DIR` to choose an
explicit directory (useful for containers and tests).

```
DARAJA_KEY=dsk_test_...
```

## Workspace

| Package | What |
|---------|------|
| `cli/` | `pesaraja` binary — init, prompt, receive, send, doctor |
| `daraja-rs/` | Rust library |

The same `DARAJA_KEY` works for both.

## Safe simulator quick start

No Daraja network access or real money is needed for the deterministic STK path:

```bash
# Terminal 1
pesaraja simulator --port 8787 --scenario success

# Terminal 2, after sandbox `pesaraja init`
pesaraja prompt --phone 254712345678 --amount 100 \
  --reference DEMO-1 --description "Simulator demo" \
  --idempotency-key demo:stk:1 --base-url http://127.0.0.1:8787 --yes
pesaraja payments list
```

Production money-moving commands require a stable caller-provided idempotency
key. Sandbox CLI commands generate and persist one when omitted.

## Using the same idempotency key

Use the same idempotency key only for the exact same business operation. An
unknown network outcome must be reconciled before retrying with a new key.

Callbacks are untrusted input. Parsing or matching a callback does not
authenticate its sender; reconcile payment state before fulfilling an order.

## Commands

| Command | What it does |
|---------|-------------|
| `pesaraja init` | Interactive setup wizard |
| `pesaraja setup <type>` | Add a paybill, till, or pochi profile |
| `pesaraja use <profile>` | Select the active profile |
| `pesaraja prompt` | STK Push (alias: `stk`) |
| `pesaraja prompt-status` | Check STK result |
| `pesaraja receive` | Local C2B callback server (sandbox) |
| `pesaraja simulate` | Fake incoming PayBill payment (sandbox) |
| `pesaraja send` | B2C payment |
| `pesaraja query` | Transaction status |
| `pesaraja balance` | Account balance |
| `pesaraja refund` | Transaction reversal |
| `pesaraja register` | Register C2B URLs |
| `pesaraja doctor` | Check config and certs |

## License

MIT. See [LICENSE](LICENSE).

See [CONTRIBUTING.md](CONTRIBUTING.md), [SUPPORT.md](SUPPORT.md), and
[SECURITY.md](SECURITY.md) before participating or reporting a problem.
