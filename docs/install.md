# Install

## Requirements

- Rust and Cargo for the CLI build path.
- Node.js 22 or 24 for the TypeScript SDK.
- A Safaricom Daraja app with consumer key and consumer secret.
- A PayBill or Till shortcode. Phone numbers are rejected where a BusinessShortCode is required.

## Install The CLI From Source

```bash
git clone https://github.com/kyambuthia/a-daraja-cli
cd a-daraja-cli
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

The installer uses a checksum-verified prebuilt release only when
`PESARAJA_VERSION` is set. If the artifact or checksum is unavailable, it
falls back to `cargo install` pinned to that release tag.

```bash
curl -fsSL https://raw.githubusercontent.com/kyambuthia/a-daraja-cli/trunk/install.sh | \
  PESARAJA_VERSION=v0.1.0 sh
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
npm install /path/to/a-daraja-cli/daraja-ts
```

The package exports ESM from `@pesaraja/sdk`.

## Configuration directory

The CLI and both SDKs use the operating system's conventional configuration
directory: `$XDG_CONFIG_HOME/daraja` (or `~/.config/daraja`) on Unix and
`%APPDATA%\daraja` on Windows. `DARAJA_CONFIG_DIR` overrides this location.

## Verify The Install

```bash
pesaraja --help
pesaraja init --help
pesaraja doctor --help
```
