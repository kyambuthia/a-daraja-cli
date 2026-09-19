# A Daraja CLI Agent Guide

## Project shape

A Daraja CLI is a Rust + TypeScript monorepo for Safaricom Daraja/M-Pesa:

- `daraja-rs/` — Rust SDK and Daraja API primitives.
- `cli/` — the `pesaraja` Rust CLI.
- `daraja-ts/` — the `@pesaraja/sdk` TypeScript SDK.
- `docs/` and `API.md` — user, architecture, and protocol documentation.

Read the relevant package code and tests before changing behavior. Design documents describe intent and may be ahead of implementation; code, tests, and current user-facing documentation are the contract.

## Working modes

Use the lightweight path for isolated documentation edits, refactors with no behavior change, and focused unit-test fixes: inspect the relevant files, edit them, and run the narrowest applicable check.

Use the full research/isolated-worker workflow for changes involving authentication, encryption, credential storage, payment requests, callbacks, idempotency, certificates, public SDK APIs, dependencies, CI, releases, or deployment. These changes require a focused review and package-level verification.

Do not use real Daraja credentials, production endpoints, or live money-moving operations. Prefer mocks, fixtures, `wiremock`, and dry-run behavior. Never print, log, commit, or paste secrets.

## Security and payment invariants

- Treat all payment commands as money-moving unless explicitly in dry-run or a test fixture.
- Preserve credential redaction, encrypted-at-rest storage, file permissions, and zeroization behavior.
- Never weaken callback validation or tell users a callback is trusted unless the implementation provides the required cryptographic verification.
- Preserve idempotency and safe retry semantics; review changes for duplicate-charge and replay risks.
- Validate environment, phone numbers, amounts, callback URLs, and required credentials at the boundary.
- Keep sandbox and production configuration visibly distinct. Production behavior must not silently fall back to sandbox behavior.
- Do not add telemetry or error output that can reveal credentials, passkeys, tokens, security credentials, or full payment payloads.

Current limitation: `verifyCallback` and `verify_callback` always fail closed
because cryptographic callback authentication is not implemented. Do not treat
callback parsing, matching, or authenticator-header presence as authentication
without an explicit implementation, test, and documentation change.

## Repository conventions

- Keep Rust and TypeScript SDK behavior aligned where the APIs overlap.
- Preserve the existing public names and command behavior unless the task explicitly requests a breaking change.
- Update relevant docs and examples when CLI flags, SDK methods, environment variables, paths, or security guarantees change.
- Resolve naming inconsistencies deliberately: the shipped binary is `pesaraja`; do not introduce `mpesa` aliases or rename commands without checking current code, tests, and docs.
- Keep patches narrow and avoid unrelated formatting or dependency churn.

## Verification

Run only checks relevant to the changed area; use the repository's configured toolchain.

Rust:

```bash
cargo test --workspace
cargo check --workspace
```

TypeScript:

```bash
npm test
npm run build
```

For a focused change, prefer package- or test-level commands over the full workspace. Add or update tests for request serialization, error handling, credential boundaries, callback parsing/verification, idempotency, and CLI validation as applicable.

Before handoff, report the exact checks run and any checks skipped. If an external Daraja behavior is uncertain or has changed, consult current primary Safaricom/Daraja documentation and record the applicable assumption in the change or its docs.

## Git safety

Inspect the worktree before editing and preserve unrelated user changes. Do not reset, stash, force-push, deploy, or commit unless explicitly authorized. Review the final diff for secrets, generated artifacts, unrelated changes, and accidental API or security regressions.
