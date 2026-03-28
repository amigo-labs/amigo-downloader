#!/bin/bash
set -euo pipefail

# amigo-dl installer — downloads the latest nightly CLI binary.
# Usage: curl -fsSL https://raw.githubusercontent.com/amigo-labs/amigo-downloader/main/scripts/install.sh | bash

REPO="amigo-labs/amigo-downloader"
INSTALL_DIR="${AMIGO_INSTALL_DIR:-/usr/local/bin}"
TAG="${AMIGO_TAG:-nightly}"  # override with AMIGO_TAG=nightly-20260328

# Detect OS and architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Linux)  OS_NAME="linux" ;;
    Darwin) OS_NAME="macos" ;;
    *)      echo "Unsupported OS: $OS"; exit 1 ;;
esac

case "$ARCH" in
    x86_64|amd64)  ARCH_NAME="x86_64" ;;
    aarch64|arm64) ARCH_NAME="aarch64" ;;
    *)             echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

ARCHIVE="amigo-dl-${OS_NAME}-${ARCH_NAME}.tar.gz"
URL="https://github.com/${REPO}/releases/download/${TAG}/${ARCHIVE}"
SHA_URL="${URL}.sha256"

echo "Installing amigo-dl (${OS_NAME}/${ARCH_NAME})..."

# Download to temp dir
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

echo "Downloading ${URL}..."
curl -fsSL -o "${TMP_DIR}/${ARCHIVE}" "$URL"
curl -fsSL -o "${TMP_DIR}/${ARCHIVE}.sha256" "$SHA_URL"

# Verify checksum
echo "Verifying checksum..."
cd "$TMP_DIR"
if command -v sha256sum &>/dev/null; then
    sha256sum -c "${ARCHIVE}.sha256"
elif command -v shasum &>/dev/null; then
    shasum -a 256 -c "${ARCHIVE}.sha256"
else
    echo "Warning: no sha256sum/shasum found, skipping verification"
fi

# Extract and install
tar xzf "$ARCHIVE"

if [ -w "$INSTALL_DIR" ]; then
    mv amigo-dl "$INSTALL_DIR/amigo-dl"
else
    echo "Installing to ${INSTALL_DIR} (requires sudo)..."
    sudo mv amigo-dl "$INSTALL_DIR/amigo-dl"
fi

chmod +x "$INSTALL_DIR/amigo-dl"

echo ""
echo "Installed amigo-dl to ${INSTALL_DIR}/amigo-dl"
echo "Run 'amigo-dl --help' to get started."
