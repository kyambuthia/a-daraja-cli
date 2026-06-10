# pesaraja

> Daraja ni ngumu. Hii sio. Pesa raja.

Safaricom's [Daraja API](https://developer.safaricom.co.ke) is a bridge. A broken one. This is the money bridge.

## The problem

To send a single M-Pesa payment via Daraja you need:

- A consumer key and consumer secret
- An OAuth token (that expires in 3600 seconds)
- A passkey (which isn't the same thing as the secret)
- A base64-encoded password made from shortcode + passkey + timestamp
- The correct environment (sandbox vs production)
- The correct URLs (which differ per product)
- Headers. So many headers.
- And god help you if the timestamp format is wrong.

## The fix

```bash
# Send money to a phone number
mpesa send --to 254712345678 --amount 100

# Query transaction status
mpesa status --id QEJ78H8JKR

# Check account balance
mpesa balance

# Initiate STK push
mpesa stk --phone 254712345678 --amount 500 --description "Moringa seedlings"
```

That's it. One command. The CLI handles OAuth tokens, password generation, expiry, retries, and all the ceremony Daraja demands.

## Install

```bash
# Coming soon
git clone https://github.com/kyambuthia/pesaraja
cd pesaraja
./install.sh
```

## Setup

```bash
mpesa init
```

Walks you through:
1. Creating a Safaricom Developer account (if you don't have one)
2. Creating a Daraja app
3. Getting your consumer key, secret, and passkey
4. Storing them locally (encrypted, not in your `.bash_history` like last time)

## Commands (planned)

| Command | What it does |
|---------|-------------|
| `mpesa init` | Interactive setup wizard |
| `mpesa send` | B2C payment |
| `mpesa stk` | STK Push (Lipa Na M-Pesa Online) |
| `mpesa status` | Query transaction status |
| `mpesa balance` | Account balance |
| `mpesa reverse` | Transaction reversal |
| `mpesa register` | C2B register URLs |
| `mpesa webhook` | Test your validation/confirmation endpoints |

## Why not just use the Daraja SDK?

Because the SDK still expects you to understand OAuth flow, hand-roll the password, manage tokens yourself, and debug with cryptic `500 Internal Server Error` responses. This CLI does all that. You just want to send money.

## Name

Temporary. The real name will be a Swahili word or phrase. Candidates:

- **madaraja** (bridges, plural) — daraja is one broken bridge, here are your bridges
- **vusha** (to cross, to transfer) — double meaning
- **kivuko** (ferry) — when the bridge is out, take the ferry
- (suggestions welcome)

## License

MIT
