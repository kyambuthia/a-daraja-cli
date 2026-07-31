# Release Process

Releases are created from version tags after the repository is clean and the
full local verification suite passes.

## Prepare

1. Update the version in `daraja-rs/Cargo.toml`, `cli/Cargo.toml`, and
   `daraja-ts/package.json`.
2. Refresh `Cargo.lock` and `package-lock.json` when required.
3. Update user-facing documentation and release notes.
4. Run the commands listed in the repository verification section below.
5. Commit the version and documentation changes, then create an annotated tag
   matching the shared version, such as `v0.1.0`.

The release workflow rejects a tag unless all three package versions match it.

## Automated Artifacts

Pushing a matching `v*` tag runs `.github/workflows/release.yml`. It repeats
the Rust and TypeScript checks, then creates or updates a GitHub release with:

- a Linux x86-64 `pesaraja` binary archive;
- a `daraja-rs` crate archive;
- an `@pesaraja/sdk` npm package archive; and
- a SHA-256 checksum beside every artifact.

CI smoke-installs the packed npm artifact and the release workflow verifies
that CLI archives contain only the expected binary and that package archives
contain the expected entry points. GitHub Actions dependencies are pinned to
immutable revisions.

The workflow builds CLI archives for Linux x86-64, Linux ARM64, macOS x86-64,
and macOS ARM64. The workflow does not publish to crates.io or npm. Registry publishing should
remain a separate, explicitly authorized step with trusted publishing or
short-lived credentials configured for the repository.

Other platforms use the installer's source-build fallback pinned to the selected
release tag.

## Verification

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace --locked
cargo test --workspace --locked
cargo package -p daraja-rs --locked
npm ci
npm run build
npm test
npm pack -w @pesaraja/sdk --dry-run
npm audit --omit=dev --audit-level=high
sh -n install.sh
```

Never use live Daraja credentials or perform live money-moving operations as
part of release verification.
