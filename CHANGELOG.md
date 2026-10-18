# Changelog

Notable changes to A Daraja CLI are recorded here. This project follows
[Semantic Versioning](https://semver.org/) once public packages are released.

## Unreleased

### Added

- Durable STK, B2C, and reversal ledger records with guarded lifecycle events.
- Stable JSON error codes and CLI simulator integration coverage.
- Package, dependency-policy, release, and community infrastructure.

### Changed

- Money-moving SDK calls require caller-provided idempotency keys.
- Ambiguous provider outcomes and local persistence failures fail closed.
- Configuration storage follows platform conventions and supports
  `DARAJA_CONFIG_DIR`.
- The `pesaraja` CLI monolith (`cli/src/main.rs`) is split into per-command
  modules (`args`, `creds`, `common`, `cmd_*`) with no behavior change.

### Removed

- The TypeScript SDK. The repository now ships only the `pesaraja` CLI and
  the `daraja-rs` crate.

### Security

- Production endpoint overrides are rejected.
- Sandbox certificate downloads are pinned and checksum/X.509 validated.
- Updated `rustls` and `quinn-proto` to clear RUSTSEC-2026-0338 to 0342 and
  RUSTSEC-2026-0346 to 0349.
