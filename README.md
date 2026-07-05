# pesaraja

> Daraja ni ngumu. Hii sio. Pesa raja.

Current code-backed docs:

- [Install](docs/install.md)
- [Setup](docs/setup.md)
- [Usage](docs/usage.md)

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
git clone https://github.com/kyambuthia/pesaraja
cd pesaraja
cargo build --release -p cli
cp target/release/pesaraja ~/.local/bin/
```

Or use `./install.sh` when a GitHub release is available.

## Setup

```bash
pesaraja init
```

Walks you through Daraja credentials, encrypts them to `~/.config/daraja/credentials.enc`, and prints your SDK key:

```
DARAJA_KEY=dsk_test_...
```

## Monorepo

| Package | What |
|---------|------|
| `cli/` | `pesaraja` binary — init, prompt, receive, send, doctor |
| `daraja-rs/` | Rust library |
| `daraja-ts/` | `@pesaraja/sdk` — TypeScript / Node / Next.js |

Same `DARAJA_KEY` works in every language.

## TypeScript / website

```typescript
import { Daraja, parseStkCallback, stkCallbackPaid } from "@pesaraja/sdk";

const d = await Daraja.fromEnv();
await d.prompt({
  phone: "254712345678",
  amount: 500,
  callbackUrl: "https://your-server.com/api/mpesa/callback",
});
```

Callback route examples: `daraja-ts/examples/nextjs-app-router/` and `daraja-ts/examples/express/`.

Install from the monorepo (npm publish not yet):

```bash
npm install file:./daraja-ts
```

## Commands

| Command | What it does |
|---------|-------------|
| `pesaraja init` | Interactive setup wizard |
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

MIT
