#!/bin/bash
# Script to run repro008 example in a tmux session and capture output
# Usage: ./run_repro.sh <N>
#   where N is the argument to pass to repro008 (e.g., 1 or 2)

set -e

if [ $# -ne 1 ]; then
  echo "Usage: $0 <N>"
  echo "  N=1: viewport = rows - 1 (should show corruption)"
  echo "  N=2: viewport = rows - 2 (should NOT show corruption)"
  exit 1
fi

N=$1
SESSION_NAME="repro_test_${N}"
SOCKET_NAME="repro_socket_$$"

# Cleanup function
cleanup() {
  tmux -L "$SOCKET_NAME" kill-session -t "$SESSION_NAME" 2>/dev/null || true
}
trap cleanup EXIT

echo "=========================================="
echo "Running repro008 with argument: $N"
echo "Viewport will be: (terminal_rows - $N)"
echo "=========================================="
echo ""

# Create tmux session with 80x24 dimensions
tmux -L "$SOCKET_NAME" new-session -d -s "$SESSION_NAME" -x 80 -y 24

# Send the cargo command
tmux -L "$SOCKET_NAME" send-keys -t "$SESSION_NAME" "cargo run --example repro008 -- $N 2>&1" Enter

# Wait for the program to run and generate output
sleep 8

# Send 'q' to quit the program
tmux -L "$SOCKET_NAME" send-keys -t "$SESSION_NAME" q

# Wait for program to exit
sleep 1

echo "--- Captured Output (visible pane): ---"
tmux -L "$SOCKET_NAME" capture-pane -t "$SESSION_NAME" -p -e

echo ""
echo "--- Captured Output (with scrollback): ---"
tmux -L "$SOCKET_NAME" capture-pane -t "$SESSION_NAME" -p -e -S -1000

echo ""
echo "=========================================="
echo "Test complete"
echo "=========================================="
