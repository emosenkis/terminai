#!/usr/bin/env bash
#
# Mac OS installation script for Termin.AI
#
# Usage:
#   ./scripts/install-macos.sh
#
# This script builds and installs Termin.AI on Mac OS.
# It requires Rust to be installed.
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo_info() {
  echo -e "${GREEN}[INFO]${NC} $1"
}

echo_warn() {
  echo -e "${YELLOW}[WARN]${NC} $1"
}

echo_error() {
  echo -e "${RED}[ERROR]${NC} $1"
}

# Check if running on Mac
if [[ "$(uname)" != "Darwin" ]]; then
  echo_error "This script is for Mac OS only. Use scripts/build-unix.sh for Linux."
  exit 1
fi

# Check for Rust
if ! command -v cargo &> /dev/null; then
  echo_error "Rust is not installed. Install from: https://rustup.rs/"
  exit 1
fi

echo_info "Rust version: $(rustc --version)"

# Build the Rust binary (only terminai, not termcap test utility)
echo_info "Building Rust binary..."
cargo build --release -p termin --bin terminai

# Determine install location
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local}"
BIN_DIR="$INSTALL_DIR/bin"
LIB_DIR="$INSTALL_DIR/lib/terminai"

echo_info "Installing to: $INSTALL_DIR"

# Create directories
mkdir -p "$BIN_DIR"

# Copy the binary
echo_info "Installing binary to $BIN_DIR/terminai..."
cp target/release/terminai "$BIN_DIR/terminai"
chmod +x "$BIN_DIR/terminai"

echo ""
echo_info "Installation complete!"
echo ""
echo "Termin.AI has been installed to: $BIN_DIR/terminai"
echo ""

# Check if BIN_DIR is in PATH
if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
  echo_warn "$BIN_DIR is not in your PATH."
  echo "Add this to your ~/.zshrc or ~/.bashrc:"
  echo ""
  echo "  export PATH=\"$BIN_DIR:\$PATH\""
  echo ""
fi

echo "To use Termin.AI, install and authenticate a supported CLI agent:"
echo ""
echo "  codex login"
echo "  # or"
echo "  claude auth"
echo ""
echo "Then run: terminai"
echo ""
echo "Press Ctrl+Space to activate the AI assistant overlay."
