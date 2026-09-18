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
- Node.js support starts at Node 22.
- Configuration storage follows platform conventions and supports
  `DARAJA_CONFIG_DIR`.

### Security

- Production endpoint overrides are rejected.
- SDK credentials are no longer enumerable TypeScript client state.
- Sandbox certificate downloads are pinned and checksum/X.509 validated.
