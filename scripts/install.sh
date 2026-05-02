#!/bin/bash
set -euo pipefail

# amigo-dl installer — downloads the latest stable CLI binary.
# Usage: curl -fsSL https://raw.githubusercontent.com/amigo-labs/amigo-downloader/main/scripts/install.sh | bash

REPO="amigo-labs/amigo-downloader"
INSTALL_DIR="${AMIGO_INSTALL_DIR:-/usr/local/bin}"
TAG="${AMIGO_TAG:-latest}"  # override with AMIGO_TAG=v0.2.0

# Resolve "latest" to the actual release tag via GitHub API
if [ "$TAG" = "latest" ]; then
    API_URL="https://api.github.com/repos/${REPO}/releases/latest"
    RESPONSE_FILE="$(mktemp)"
    trap 'rm -f "$RESPONSE_FILE"' EXIT
    HTTP_STATUS=$(curl -sSL -o "$RESPONSE_FILE" -w '%{http_code}' "$API_URL" || echo "000")
    case "$HTTP_STATUS" in
        200) ;;
        404)
            echo "No stable release exists yet for ${REPO}. Set AMIGO_TAG=<tag> to install a specific version." >&2
            exit 1
            ;;
        000)
            echo "Failed to reach GitHub API at ${API_URL}. Check your network connection." >&2
            exit 1
            ;;
        *)
            echo "GitHub API returned HTTP ${HTTP_STATUS} for ${API_URL}." >&2
            exit 1
            ;;
    esac
    TAG=$(grep '"tag_name"' "$RESPONSE_FILE" | head -1 | sed 's/.*"\(.*\)".*/\1/')
    rm -f "$RESPONSE_FILE"
    trap - EXIT
    if [ -z "$TAG" ]; then
        echo "Could not parse tag_name from GitHub API response." >&2
        exit 1
    fi
    echo "Latest stable release: ${TAG}"
fi

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
