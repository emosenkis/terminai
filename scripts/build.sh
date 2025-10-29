#!/bin/bash

# Only run in remote environments (not locally)
if [ "$CLAUDE_CODE_REMOTE" != "true" ]; then
  exit 0
fi

echo "Building Termin.AI in remote environment..."

# Build the Rust project
cargo build

echo "Build complete!"
exit 0
