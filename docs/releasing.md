# Release Process

Releases are created from version tags after the repository is clean and the
full local verification suite passes.

## Preflight

Check an existing `vX.Y.Z` tag before releasing. The script never creates
tags or publishes anything:

```bash
scripts/release-preflight.sh v0.1.0
scripts/release-preflight.sh --verify v0.1.0
```

It rejects a dirty checkout, requires `daraja-rs`, `cli`, and
`@pesaraja/sdk` versions to equal the tag, and requires the tag to resolve
to `HEAD`. With `--verify`, it also runs the verification commands below
using locked/package options.

## Prepare

1. Update the version in `daraja-rs/Cargo.toml`, `cli/Cargo.toml`, and
   `daraja-ts/package.json`.
2. Refresh `Cargo.lock` and `package-lock.json` when required.
3. Move relevant entries from `CHANGELOG.md`'s Unreleased section into the new
   version and update user-facing documentation.
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

The release workflow also creates GitHub/Sigstore build-provenance attestations
for the release assets. Verify a downloaded asset with `gh attestation verify
<asset> --repo kyambuthia/a-daraja-cli`.

CI smoke-installs the packed npm artifact and the release workflow verifies
that CLI archives contain only the expected binary and that package archives
contain the expected entry points. GitHub Actions dependencies are pinned to
immutable revisions.

The workflow builds CLI archives for Linux x86-64, Linux ARM64, macOS x86-64,
and macOS ARM64. It does not publish to a registry. The separate manual
`publish.yml` workflow requires the protected `npm-publish` environment, an
explicit `publish` confirmation, and npm trusted publishing via GitHub OIDC.
Configure the npm package's trusted publisher for this repository and workflow
before enabling it. crates.io publishing remains a separately authorized step.

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
npm run lint:package -w @pesaraja/sdk
npm pack -w @pesaraja/sdk --dry-run
npm audit --omit=dev --audit-level=high
sh -n install.sh
sh -n scripts/release-preflight.sh
```

Never use live Daraja credentials or perform live money-moving operations as
part of release verification.
