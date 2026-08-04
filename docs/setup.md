# Setup

## Interactive Setup

Run:

```bash
pesaraja init
```

The wizard asks for:

- environment: `sandbox` or `production`
- Daraja consumer key
- Daraja consumer secret
- PayBill or Till shortcode
- optional initiator name and initiator password for send, query, balance, and refund

Sandbox setup uses the fixed public sandbox passkey automatically. Production setup asks for the Lipa Na M-Pesa passkey.

After setup, the CLI prints an SDK key:

```bash
DARAJA_KEY=dsk_test_...
```

The CLI also saves the key locally, so normal CLI commands do not require `DARAJA_KEY` to be exported.

The local key and encrypted credential blob are stored under the same OS user.
Encryption protects against accidental disclosure and other users when file
permissions are enforced; it does not protect against compromise of that user
account. For deployed production applications, inject the key through a secret
manager rather than copying the local key file.

## Non-Interactive Setup

Sandbox:

```bash
DARAJA_CONSUMER_SECRET="$DARAJA_CONSUMER_SECRET" pesaraja init \
  --env sandbox \
  --consumer-key "$DARAJA_CONSUMER_KEY" \
  --shortcode 174379 \
  --non-interactive
```

Sandbox uses the fixed public passkey. The consumer secret is read from
`DARAJA_CONSUMER_SECRET`, never from a command-line argument.

Production:

```bash
DARAJA_CONSUMER_SECRET="$DARAJA_CONSUMER_SECRET" \
DARAJA_PASSKEY="$DARAJA_PASSKEY" \
DARAJA_INITIATOR_PASSWORD="$DARAJA_INITIATOR_PASSWORD" \
pesaraja init \
  --env production \
  --consumer-key "$DARAJA_CONSUMER_KEY" \
  --shortcode "$DARAJA_SHORTCODE" \
  --initiator-name "$DARAJA_INITIATOR_NAME" \
  --non-interactive
```

Non-interactive setup reads secrets from `DARAJA_CONSUMER_SECRET` and, for
production, `DARAJA_PASSKEY`. If `--initiator-name` is provided,
`DARAJA_INITIATOR_PASSWORD` is also required. Secret-valued CLI flags are not
supported because shell history and process listings can expose them.

## Stored Files

`pesaraja init` writes under `~/.config/daraja/`:

- `key`: local SDK key, written owner-readable.
- `credentials.enc`: encrypted credential blob used by SDK constructors.
- `profiles.json`: active profile registry metadata.
- `profiles/personal.enc`: encrypted credentials for the active personal profile.
- `sandbox.cer`: sandbox API certificate when sandbox setup succeeds.
- `payments.db`: local payment lifecycle records and immutable event history.

The CLI creates the configuration directory with mode `0700` and private files
with mode `0600` on Unix. It refuses to load a key or encrypted credential file
that is accessible to other users. In CI or deployed environments, prefer
`DARAJA_KEY` from a secret manager instead of copying the local key file.

For production B2C, transaction status, balance, and reversal operations, save the production certificate as:

```bash
~/.config/daraja/production.cer
```

or set:

```bash
export DARAJA_CERT_PATH=/absolute/path/to/production.cer
```

## Environment Variables

`DARAJA_KEY` overrides the local key file:

```bash
export DARAJA_KEY="$(pesaraja export-key)"
```

Use this in CI, Docker, and deployed apps that need the SDK key.

## Inspect Setup

Show non-secret config:

```bash
pesaraja config
pesaraja --json config
```

Run pre-flight checks:

```bash
pesaraja doctor
pesaraja --json doctor
```

`doctor` checks key presence, encrypted credentials, OAuth, initiator credentials, certificates, and environment-specific warnings.

## Current Profile Model

The CLI creates and uses an active `personal` profile. Add merchant profiles
without replacing it:

```bash
pesaraja setup paybill --name shop
pesaraja setup till --name kiosk
pesaraja setup pochi --name wallet
pesaraja use shop
pesaraja --profile kiosk balance --result-url https://example.com/result --timeout-url https://example.com/timeout
```

`--profile` selects a profile for one invocation. `pesaraja use` changes the
persistent active profile. Profile metadata appears in `pesaraja config`.

Setup uses the current SDK key's environment. Initialize the requested
environment first if a merchant profile needs a different environment.
