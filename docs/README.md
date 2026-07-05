# Pesaraja Docs

These docs describe the behavior implemented in the current codebase.

Start here:

- [Install](install.md): install the CLI and TypeScript SDK.
- [Setup](setup.md): run `pesaraja init`, understand stored credentials, and check your environment.
- [Usage](usage.md): collect payments, receive callbacks, send money, query status, check balance, and use the SDK.

Current scope:

- The CLI stores one active `personal` profile created by `pesaraja init`.
- Profile storage exists, but user-facing `setup`, `use`, and `--profile` flows are not implemented yet.
- The TypeScript SDK is available from the monorepo package, not from a published npm package unless that release process has happened separately.
- `verifyCallback` currently checks callback shape/header presence. Treat it as a guard helper, not complete cryptographic callback verification.
