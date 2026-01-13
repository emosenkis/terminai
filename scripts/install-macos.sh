#!/usr/bin/env bash
#
# Mac OS installation script for Termin.AI
#
# Usage:
#   ./scripts/install-macos.sh
#
# This script builds and installs Termin.AI on Mac OS.
# It requires Rust, Python 3.11+, and uv to be installed.
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

# Check for Python 3.11+
if ! command -v python3 &> /dev/null; then
  echo_error "Python 3 is not installed. Install from: https://www.python.org/"
  exit 1
fi

PYTHON_VERSION=$(python3 -c 'import sys; print(".".join(map(str, sys.version_info[:2])))')
PYTHON_MAJOR=$(python3 -c 'import sys; print(sys.version_info.major)')
PYTHON_MINOR=$(python3 -c 'import sys; print(sys.version_info.minor)')

echo_info "Python version: $PYTHON_VERSION"

if [[ "$PYTHON_MAJOR" -lt 3 ]] || [[ "$PYTHON_MAJOR" -eq 3 && "$PYTHON_MINOR" -lt 11 ]]; then
  echo_error "Python 3.11 or higher is required (found $PYTHON_VERSION)"
  exit 1
fi

# Check for uv
if ! command -v uv &> /dev/null; then
  echo_warn "uv is not installed. Installing via Homebrew..."
  if ! command -v brew &> /dev/null; then
    echo_error "Homebrew is not installed. Install from: https://brew.sh/"
    exit 1
  fi
  brew install uv
fi

echo_info "uv version: $(uv --version)"

# Build the Rust binary
echo_info "Building Rust binary..."
cargo build --release -p termin

# Determine install location
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local}"
BIN_DIR="$INSTALL_DIR/bin"
LIB_DIR="$INSTALL_DIR/lib/terminai"
PYTHON_DIR="$LIB_DIR/python"

echo_info "Installing to: $INSTALL_DIR"

# Create directories
mkdir -p "$BIN_DIR"
mkdir -p "$PYTHON_DIR"

# Copy the binary
echo_info "Installing binary to $BIN_DIR/terminai..."
cp target/release/terminai "$BIN_DIR/terminai"
chmod +x "$BIN_DIR/terminai"

# Copy Python agent
echo_info "Installing Python agent to $PYTHON_DIR..."
cp -r python/terminai_agent "$PYTHON_DIR/"
cp python/pyproject.toml "$PYTHON_DIR/"
[ -f python/README.md ] && cp python/README.md "$PYTHON_DIR/"

# Install Python dependencies
echo_info "Installing Python dependencies with uv..."
cd "$PYTHON_DIR"
uv sync --frozen

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

echo "To use Termin.AI, you need to set an API key:"
echo ""
echo "  export ANTHROPIC_API_KEY=\"sk-ant-...\"  # For Claude"
echo "  export OPENAI_API_KEY=\"sk-...\"         # For GPT-4"
echo "  export GEMINI_API_KEY=\"...\"            # For Gemini"
echo "  export OPENROUTER_API_KEY=\"sk-or-...\"  # For OpenRouter"
echo ""
echo "Then run: terminai"
echo ""
echo "Press Ctrl+Space to activate the AI assistant overlay."
