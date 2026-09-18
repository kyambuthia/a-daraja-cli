#!/bin/sh
set -eu

# pesaraja installer — one command to get the M-Pesa CLI
# curl -fsSL https://raw.githubusercontent.com/kyambuthia/a-daraja-cli/trunk/install.sh | bash

BOLD=""; GREEN=""; YELLOW=""; RED=""; NC=""
if [ -t 1 ]; then
    BOLD="$(printf '\033[1m')"
    GREEN="$(printf '\033[32m')"
    YELLOW="$(printf '\033[33m')"
    RED="$(printf '\033[31m')"
    NC="$(printf '\033[0m')"
fi

INSTALL_DIR="${HOME}/.local/bin"
BIN="${INSTALL_DIR}/pesaraja"

info()  { printf '%s  %s%s\n' "${GREEN}✓${NC}" "$*"; }
warn()  { printf '%s  %s%s\n' "${YELLOW}⚠${NC}" "$*"; }
err()   { printf '%s  %s%s\n' "${RED}×${NC}" "$*"; }

# Detect platform
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"
case "$ARCH" in
    x86_64|amd64) ARCH="x86_64" ;;
    aarch64|arm64) ARCH="aarch64" ;;
    *) err "Unsupported architecture: $ARCH"; exit 1 ;;
esac

REPO="kyambuthia/a-daraja-cli"
VERSION="${PESARAJA_VERSION:-}"
TARBALL="pesaraja-${VERSION}-${OS}-${ARCH}.tar.gz"
RELEASE_URL="https://github.com/${REPO}/releases/download/${VERSION}/${TARBALL}"
BUILD_FROM_SOURCE=0
INSTALLED_BIN_DIR="$INSTALL_DIR"
TMP_DIR=""

cleanup() {
    if [ -n "$TMP_DIR" ] && [ -d "$TMP_DIR" ]; then
        rm -rf "$TMP_DIR"
    fi
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

verify_checksum() {
    checksum_file="$1"
    if command -v sha256sum >/dev/null 2>&1; then
        (cd "$TMP_DIR" && sha256sum -c "$checksum_file" >/dev/null 2>&1)
    elif command -v shasum >/dev/null 2>&1; then
        (cd "$TMP_DIR" && shasum -a 256 -c "$checksum_file" >/dev/null 2>&1)
    else
        return 1
    fi
}

mkdir -p "$INSTALL_DIR"

info "Detected ${OS}/${ARCH}"

# Try a prebuilt binary only when an explicit release version is requested.
if [ -n "$VERSION" ] \
    && command -v curl >/dev/null 2>&1 \
    && command -v tar >/dev/null 2>&1 \
    && (command -v sha256sum >/dev/null 2>&1 || command -v shasum >/dev/null 2>&1); then
    TMP_DIR="$(mktemp -d 2>/dev/null || mktemp -d -t pesaraja)"
    if curl -fsSL "$RELEASE_URL" -o "${TMP_DIR}/${TARBALL}" 2>/dev/null \
        && curl -fsSL "${RELEASE_URL}.sha256" -o "${TMP_DIR}/${TARBALL}.sha256" 2>/dev/null \
        && verify_checksum "${TARBALL}.sha256" \
        && [ "$(tar -tzf "${TMP_DIR}/${TARBALL}" | wc -l | tr -d ' ')" = "2" ] \
        && tar -tzf "${TMP_DIR}/${TARBALL}" | grep -qx pesaraja \
        && tar -tzf "${TMP_DIR}/${TARBALL}" | grep -qx LICENSE; then
        tar -xzf "${TMP_DIR}/${TARBALL}" -C "$TMP_DIR"
        install -m 755 "${TMP_DIR}/pesaraja" "$BIN"
        info "Installed checksum-verified binary to ${BIN}"
    else
        warn "No verified prebuilt binary for ${OS}/${ARCH} — building from source"
        BUILD_FROM_SOURCE=1
    fi
else
    if [ -z "$VERSION" ]; then
        warn "No release version selected — building from source"
    else
        warn "curl, tar, and a checksum tool are required for prebuilt installs — building from source"
    fi
    BUILD_FROM_SOURCE=1
fi

# Fall back to cargo
if [ "${BUILD_FROM_SOURCE}" = "1" ]; then
    if command -v cargo >/dev/null 2>&1; then
        info "Building with cargo..."
        if [ -n "$VERSION" ]; then
            cargo install --locked --git "https://github.com/${REPO}.git" --tag "$VERSION" --bin pesaraja || {
                err "cargo install failed for tag ${VERSION}"
                exit 1
            }
        else
            cargo install --locked --git "https://github.com/${REPO}.git" --bin pesaraja || {
                err "cargo install failed"
                exit 1
            }
        fi
        INSTALLED_BIN_DIR="${CARGO_HOME:-${HOME}/.cargo}/bin"
        info "Installed via cargo to ~/.cargo/bin/pesaraja"
    else
        err "cargo not found. Install Rust: https://rustup.rs"
        err "Or download a binary from: https://github.com/${REPO}/releases"
        exit 1
    fi
fi

# Check PATH
case ":$PATH:" in
    *:"$INSTALLED_BIN_DIR":*) ;;
    *)
        warn "Add ${INSTALLED_BIN_DIR} to your PATH:"
        printf "  export PATH=\"%s:\$PATH\"\n" "$INSTALLED_BIN_DIR"
        ;;
esac

echo ""
info "${BOLD}pesaraja installed!${NC}"
echo "  Run:  ${GREEN}pesaraja init${NC}"
echo ""
