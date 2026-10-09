# Roadmap

A Daraja CLI is an alpha toolkit for integrating Safaricom Daraja safely. The
repository ships the `pesaraja` command and the `daraja-rs` crate. This
roadmap describes remaining work; it does not describe shipped behavior.

## Public preview — 0.1

- Keep CI, package linting, dependency policy, and simulator integration green.
- Enable GitHub private vulnerability reporting and Discussions.
- Publish signed/checksummed release artifacts and verify install instructions.
- Document the known callback-authentication limitation prominently.

Exit criteria: repeatable tagged builds, clean package checks, private security
intake, and no unresolved critical/high dependency advisories.

## Payment lifecycle — 0.2

- Add authoritative reconciliation for B2C and reversal callbacks/results.
- Add provider transaction IDs and result payload summaries to all ledger kinds.
- Define an operator workflow for resolving `unknown` outcomes.
- Add migration/versioning tests for the local ledger schema.

Exit criteria: every money-moving operation has a durable, inspectable lifecycle
and documented recovery path.

## Callback trust — 0.3

- Implement a documented, provider-supported callback authentication mechanism
  when Safaricom supplies a verifiable contract.
- Add replay protection, timestamp policy, and key/certificate rotation guidance.
- Keep fulfillment APIs fail-closed when verification is unavailable.

Exit criteria: applications can distinguish parsed callbacks from authenticated
provider events without relying on source IP alone.

## Stable release — 1.0

- Stabilize the public Rust API with semver compatibility checks.
- Publish an upgrade and deprecation policy.
- Complete accessibility and terminal UX review for interactive commands.
- Establish supported Rust and Node release lines and maintenance windows.

Items enter a milestone only with a user problem, safety analysis, and focused
verification plan. See [CONTRIBUTING.md](CONTRIBUTING.md).
