#!/bin/sh
# Release preflight for A Daraja CLI.
#
# Checks that a release tag is ready without creating tags,
# publishing, or modifying code:
#   - exactly one existing vX.Y.Z tag (annotated or lightweight);
#   - clean working tree;
#   - daraja-rs and cli versions both equal the tag;
#   - the tag resolves to HEAD.
#
# Usage:
#   scripts/release-preflight.sh [--verify] vX.Y.Z
#
# With --verify, also runs the release checks documented in
# docs/releasing.md using locked/package options.
set -eu

usage() {
    printf 'usage: %s [--verify] vX.Y.Z\n' "$0" >&2
}

fail() {
    printf 'release-preflight: error: %s\n' "$*" >&2
    exit 1
}

ok() {
    printf 'release-preflight: %s\n' "$*"
}

verify=0
tag=""
case "$#" in
    1)
        tag="$1"
        ;;
    2)
        if [ "$1" = "--verify" ]; then
            verify=1
            tag="$2"
        else
            usage
            exit 2
        fi
        ;;
    *)
        usage
        exit 2
        ;;
esac

case "$tag" in
    --verify|-h|--help|"")
        usage
        exit 2
        ;;
esac

case "$tag" in
    v*)
        ;;
    *)
        fail "invalid tag \"$tag\": expected format vX.Y.Z (example: v0.1.0)"
        ;;
esac

version="${tag#v}"
case "$version" in
    ""|.*|*.|*..*|*[!0-9.]*)
        fail "invalid tag \"$tag\": expected format vX.Y.Z (example: v0.1.0)"
        ;;
esac
dots="$(printf '%s' "$version" | tr -cd '.' | wc -c)"
dots="$(printf '%s' "$dots" | tr -d '[:space:]')"
if [ "$dots" != "2" ]; then
    fail "invalid tag \"$tag\": expected format vX.Y.Z (example: v0.1.0)"
fi

repo_root="$(git rev-parse --show-toplevel 2>/dev/null)" \
    || fail "not a git repository; run from inside the checkout"
cd "$repo_root"

if ! git show-ref --verify --quiet "refs/tags/$tag"; then
    fail "tag \"$tag\" does not exist locally; fetch tags first (example: git fetch --tags)"
fi

if [ -n "$(git status --porcelain)" ]; then
    fail "working tree is dirty; commit or discard changes first (check: git status --short)"
fi

cargo_pkg_version() {
    awk '
        /^\[package\]/{ in_pkg = 1; next }
        /^\[/{ in_pkg = 0 }
        in_pkg && $0 ~ /^version[[:space:]]*=/ {
            line = $0
            sub(/^[^"]*"/, "", line)
            sub(/".*$/, "", line)
            print line
            exit
        }
    ' "$1"
}

daraja_version="$(cargo_pkg_version "daraja-rs/Cargo.toml")"
cli_version="$(cargo_pkg_version "cli/Cargo.toml")"

[ -n "$daraja_version" ] || fail "could not read version from \"daraja-rs/Cargo.toml\""
[ -n "$cli_version" ] || fail "could not read version from \"cli/Cargo.toml\""

if [ "$daraja_version" != "$version" ]; then
    fail "daraja-rs version \"$daraja_version\" does not match tag \"$tag\"; update \"daraja-rs/Cargo.toml\" to \"$version\""
fi
if [ "$cli_version" != "$version" ]; then
    fail "cli version \"$cli_version\" does not match tag \"$tag\"; update \"cli/Cargo.toml\" to \"$version\""
fi
ok "versions match \"$tag\" (daraja-rs, cli)"

tag_commit="$(git rev-list -n 1 "$tag")" \
    || fail "could not resolve tag \"$tag\" to a commit"
head_commit="$(git rev-parse HEAD)" \
    || fail "could not resolve HEAD to a commit"
if [ "$tag_commit" != "$head_commit" ]; then
    fail "tag \"$tag\" resolves to \"$tag_commit\" but HEAD is \"$head_commit\"; move the tag or checkout the tagged commit first"
fi
ok "tag \"$tag\" resolves to HEAD (\"$head_commit\")"

if [ "$verify" = "1" ]; then
    ok "running: cargo fmt --all --check"
    cargo fmt --all --check
    ok "running: cargo clippy --workspace --all-targets -- -D warnings"
    cargo clippy --workspace --all-targets -- -D warnings
    ok "running: cargo check --workspace --locked"
    cargo check --workspace --locked
    ok "running: cargo test --workspace --locked"
    cargo test --workspace --locked
    ok "running: cargo package -p daraja-rs --locked"
    cargo package -p daraja-rs --locked
    ok "running: sh -n install.sh"
    sh -n install.sh
    ok "running: sh -n scripts/release-preflight.sh"
    sh -n scripts/release-preflight.sh
fi

ok "release preflight passed for \"$tag\""
