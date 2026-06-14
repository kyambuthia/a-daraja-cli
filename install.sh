#!/bin/sh
set -e

# pesaraja installer — one command to get the M-Pesa CLI
# curl -fsSL https://raw.githubusercontent.com/kyambuthia/pesaraja/trunk/install.sh | bash

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

REPO="kyambuthia/pesaraja"
VERSION="v0.1.0"
TARBALL="pesaraja-${VERSION}-${OS}-${ARCH}.tar.gz"
RELEASE_URL="https://github.com/${REPO}/releases/download/${VERSION}/${TARBALL}"

mkdir -p "$INSTALL_DIR"

info "Detected ${OS}/${ARCH}"

# Try prebuilt binary first
if command -v curl >/dev/null 2>&1; then
    if curl -fsSL "$RELEASE_URL" -o /tmp/pesaraja.tar.gz 2>/dev/null; then
        tar -xzf /tmp/pesaraja.tar.gz -C /tmp
        cp /tmp/pesaraja "$BIN"
        chmod +x "$BIN"
        rm -f /tmp/pesaraja.tar.gz /tmp/pesaraja
        info "Installed prebuilt binary to ${BIN}"
    else
        warn "No prebuilt binary for ${OS}/${ARCH} — building from source"
        BUILD_FROM_SOURCE=1
    fi
else
    BUILD_FROM_SOURCE=1
fi

# Fall back to cargo
if [ "${BUILD_FROM_SOURCE}" = "1" ]; then
    if command -v cargo >/dev/null 2>&1; then
        info "Building with cargo..."
        cargo install --git "https://github.com/${REPO}.git" --bin pesaraja 2>/dev/null || {
            err "cargo install failed"
            exit 1
        }
        info "Installed via cargo to ~/.cargo/bin/pesaraja"
    else
        err "cargo not found. Install Rust: https://rustup.rs"
        err "Or download a binary from: https://github.com/${REPO}/releases"
        exit 1
    fi
fi

# Check PATH
case ":$PATH:" in
    *:"$INSTALL_DIR":*) ;;
    *)
        warn "Add ${INSTALL_DIR} to your PATH:"
        printf "  export PATH=\"%s:\$PATH\"\n" "$INSTALL_DIR"
        ;;
esac

echo ""
info "${BOLD}pesaraja installed!${NC}"
echo "  Run:  ${GREEN}pesaraja init${NC}"
echo ""

