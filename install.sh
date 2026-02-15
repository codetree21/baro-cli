#!/bin/sh
set -eu

REPO="codetree21/baro-cli"
INSTALL_DIR="${BARO_INSTALL_DIR:-$HOME/.local/bin}"

# Detect OS
OS=$(uname -s)
case "$OS" in
  Darwin) os="apple-darwin" ;;
  Linux)  os="unknown-linux-gnu" ;;
  *)      echo "Error: unsupported OS: $OS"; exit 1 ;;
esac

# Detect architecture
ARCH=$(uname -m)
case "$ARCH" in
  x86_64|amd64)  arch="x86_64" ;;
  arm64|aarch64)  arch="aarch64" ;;
  *)              echo "Error: unsupported architecture: $ARCH"; exit 1 ;;
esac

TARGET="${arch}-${os}"

# Get latest version
echo "Fetching latest version..."
LATEST=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed -E 's/.*"v([^"]+)".*/\1/')
if [ -z "$LATEST" ]; then
  echo "Error: could not determine latest version"
  exit 1
fi

ARCHIVE="baro-${LATEST}-${TARGET}.tar.gz"
URL="https://github.com/${REPO}/releases/download/v${LATEST}/${ARCHIVE}"

echo "Installing baro v${LATEST} for ${TARGET}..."

# Download to temp directory
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT
curl -fsSL "$URL" -o "${TMPDIR}/${ARCHIVE}"

# Verify checksum
CHECKSUM_URL="https://github.com/${REPO}/releases/download/v${LATEST}/checksums.sha256"
if curl -fsSL "$CHECKSUM_URL" -o "${TMPDIR}/checksums.sha256" 2>/dev/null; then
  EXPECTED=$(grep "$ARCHIVE" "${TMPDIR}/checksums.sha256" | awk '{print $1}')
  if command -v sha256sum >/dev/null 2>&1; then
    ACTUAL=$(sha256sum "${TMPDIR}/${ARCHIVE}" | awk '{print $1}')
  elif command -v shasum >/dev/null 2>&1; then
    ACTUAL=$(shasum -a 256 "${TMPDIR}/${ARCHIVE}" | awk '{print $1}')
  else
    ACTUAL=""
  fi
  if [ -n "$ACTUAL" ] && [ -n "$EXPECTED" ] && [ "$ACTUAL" != "$EXPECTED" ]; then
    echo "Error: checksum mismatch"
    echo "  Expected: $EXPECTED"
    echo "  Actual:   $ACTUAL"
    exit 1
  fi
fi

# Extract and install
tar -xzf "${TMPDIR}/${ARCHIVE}" -C "${TMPDIR}"
mkdir -p "$INSTALL_DIR"
mv "${TMPDIR}/baro" "${INSTALL_DIR}/baro"
chmod +x "${INSTALL_DIR}/baro"

echo "Installed baro v${LATEST} to ${INSTALL_DIR}/baro"

# PATH hint
case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) ;;
  *)
    echo ""
    echo "Add ${INSTALL_DIR} to your PATH:"
    echo ""
    echo "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.zshrc"
    echo ""
    echo "Then restart your shell or run: source ~/.zshrc"
    ;;
esac

echo ""
echo "Run 'baro --help' to get started."
