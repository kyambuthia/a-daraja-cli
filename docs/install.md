# Install

## Requirements

- Rust and Cargo for the CLI build path.
- Node.js 18 or newer for the TypeScript SDK.
- A Safaricom Daraja app with consumer key and consumer secret.
- A PayBill or Till shortcode. Phone numbers are rejected where a BusinessShortCode is required.

## Install The CLI From Source

```bash
git clone https://github.com/kyambuthia/pesaraja
cd pesaraja
cargo build --release -p cli
install -m 755 target/release/pesaraja ~/.local/bin/pesaraja
```

Make sure `~/.local/bin` is on your `PATH`:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

Check that the binary is available:

```bash
pesaraja --help
```

## Install With `install.sh`

The installer first tries a prebuilt GitHub release for `v0.1.0`. If no binary exists for your platform, it falls back to `cargo install`.

```bash
curl -fsSL https://raw.githubusercontent.com/kyambuthia/pesaraja/trunk/install.sh | sh
```

If the fallback path runs, Cargo must be installed.

## Install The TypeScript SDK

From this monorepo:

```bash
npm install
npm run build -w @pesaraja/sdk
```

From another local project, install the package by path until the package is published:

```bash
npm install /path/to/pesaraja/daraja-ts
```

The package exports ESM from `@pesaraja/sdk`.

## Verify The Install

```bash
pesaraja --help
pesaraja init --help
pesaraja doctor --help
```
