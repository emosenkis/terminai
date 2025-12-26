#!/bin/bash
# Test script for Python LLM integration
#
# This script sets up the proper environment for running Rust tests
# that use PyO3 to call Python code.

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Python LLM Integration Test Runner${NC}"
echo "=========================================="
echo ""

# Find project root
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$PROJECT_ROOT"

# Set up Python environment
# Note: We use the base Python (not venv) for embedding because venvs don't
# have the full standard library needed for PyO3's auto-initialize
PYTHON_VENV="$PROJECT_ROOT/python/.venv/bin/python"
if [ ! -f "$PYTHON_VENV" ]; then
    echo -e "${RED}Error: Python venv not found at $PYTHON_VENV${NC}"
    echo "Run 'cd python && uv sync' first"
    exit 1
fi

# Get the base Python (not venv) - this is what uv uses
BASE_PYTHON=$("$PYTHON_VENV" -c "import sys; print(sys._base_executable)")
if [ -z "$BASE_PYTHON" ] || [ ! -f "$BASE_PYTHON" ]; then
    echo -e "${RED}Error: Could not find base Python executable${NC}"
    exit 1
fi

# Get Python library directory from base Python
PYTHON_LIBDIR=$("$BASE_PYTHON" -c "import sysconfig; print(sysconfig.get_config_var('LIBDIR'))")
if [ -z "$PYTHON_LIBDIR" ]; then
    echo -e "${RED}Error: Could not determine Python library directory${NC}"
    exit 1
fi

# Get Python home from base Python
PYTHON_HOME=$("$BASE_PYTHON" -c "import sys; print(sys.prefix)")

echo -e "${YELLOW}Python configuration:${NC}"
echo "  Base Python: $BASE_PYTHON"
echo "  Python home: $PYTHON_HOME"
echo "  Library dir: $PYTHON_LIBDIR"

# Check for Python shared library
if [ ! -d "$PYTHON_LIBDIR" ]; then
    echo -e "${RED}Error: Python library directory does not exist: $PYTHON_LIBDIR${NC}"
    exit 1
fi

# Load API keys from config
CONFIG_ENV="$HOME/.config/terminai/terminai.env"
if [ -f "$CONFIG_ENV" ]; then
    echo -e "${YELLOW}Loading API keys from $CONFIG_ENV${NC}"
    # Source and export all variables
    set -a  # Mark all variables for export
    source "$CONFIG_ENV"
    set +a  # Stop marking for export
else
    echo -e "${YELLOW}Warning: Config file not found: $CONFIG_ENV${NC}"
    echo "  API key tests may be skipped"
fi

# Get venv site-packages for PYTHONPATH
SITE_PACKAGES=$("$PYTHON_VENV" -c "import site; print(site.getsitepackages()[0])")

# Export environment variables for PyO3
# Use base Python for embedding (has full stdlib)
export PYO3_PYTHON="$BASE_PYTHON"
export PYTHONHOME="$PYTHON_HOME"
export LD_LIBRARY_PATH="$PYTHON_LIBDIR:${LD_LIBRARY_PATH:-}"

# Add both our Python module and venv site-packages to PYTHONPATH
# This allows the embedded Python to find both our code and installed packages
export PYTHONPATH="$PROJECT_ROOT/python:$SITE_PACKAGES:${PYTHONPATH:-}"

echo ""
echo -e "${YELLOW}Environment variables:${NC}"
echo "  PYO3_PYTHON=$PYO3_PYTHON"
echo "  PYTHONHOME=$PYTHONHOME"
echo "  LD_LIBRARY_PATH=$LD_LIBRARY_PATH"
echo "  PYTHONPATH=$PYTHONPATH"
if [ -n "$ANTHROPIC_API_KEY" ]; then
    echo "  ANTHROPIC_API_KEY=sk-ant-***${ANTHROPIC_API_KEY: -8}"
else
    echo "  ANTHROPIC_API_KEY=(not set - tests will be skipped)"
fi

echo ""
echo -e "${GREEN}Running tests...${NC}"
echo ""

# Parse command line arguments
TEST_FILTER="${1:-}"

if [ -n "$TEST_FILTER" ]; then
    echo "Running tests matching: $TEST_FILTER"
    cargo test --lib "$TEST_FILTER" -- --nocapture
else
    echo "Running all LLM client tests"
    cargo test --lib client_test -- --nocapture
fi

echo ""
echo -e "${GREEN}✓ Tests completed${NC}"
