# Contributing

Thanks for helping improve A Daraja CLI. This project handles payment flows, so
changes should be small, testable, and explicit about failure semantics.

## Development setup

Requirements are Rust 1.88.0 (pinned in rust-toolchain.toml) and Node.js 22 or 24.

```bash
cargo test --workspace --locked
npm ci
npm test
npm run build
```

Before opening a pull request, also run:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
npm run lint:package -w @pesaraja/sdk
```

Never use live Daraja credentials or initiate real money movement in tests.
Use the local simulator and synthetic values. Money-moving operations must keep
caller-provided idempotency keys, ambiguous outcome handling, and Rust/TypeScript
behavior aligned.

## Pull requests

- Explain the user-visible problem and the chosen behavior.
- Add focused tests, including failure paths.
- Update public docs and both SDKs when their contract changes.
- Do not commit secrets, generated credentials, certificates, or payment data.
- Use a short imperative commit subject, such as `Harden payment retries`.

Security issues belong in a private advisory; see [SECURITY.md](SECURITY.md).
