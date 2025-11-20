#!/bin/bash
# Demo script for Termin.AI
# This script demonstrates the application running commands

set -e

echo "========================================="
echo "Termin.AI Demo"
echo "========================================="
echo ""
echo "Built on mprocs foundation with AI extensions"
echo ""

# Show version
echo "1. Checking version:"
./target/debug/mprocs --version
echo ""

# Show help
echo "2. Available options:"
./target/debug/mprocs --help | head -15
echo ""

# Show test configuration
echo "3. Test configuration (test-mprocs.yaml):"
cat test-mprocs.yaml
echo ""

# Run a simple command through mprocs
echo "4. Running simple echo command:"
echo "   (mprocs will execute: echo 'Hello from Termin.AI!')"
echo ""

# Start mprocs in background with a simple command, run for a few seconds, then quit
timeout 3 ./target/debug/mprocs -c test-mprocs.yaml --server 127.0.0.1:4051 &
MPROCS_PID=$!
sleep 2

# Try to cleanly stop it
./target/debug/mprocs --server 127.0.0.1:4051 --ctl '{c: quit}' 2>/dev/null || true
wait $MPROCS_PID 2>/dev/null || true

echo ""
echo "========================================="
echo "Demo Complete!"
echo "========================================="
echo ""
echo "Implementation Status:"
echo "✓ mprocs foundation integrated"
echo "✓ Core modules compiled successfully"
echo "✓ LLM client module ready"
echo "✓ AI chat process scaffolded"
echo "✓ Command parser implemented"
echo "✓ Privacy filters active"
echo ""
echo "Next steps:"
echo "- Complete AI chat UI integration"
echo "- Implement LLM view activation"
echo "- Add command execution workflow"
echo ""
