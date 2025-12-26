#!/bin/bash
# Convenience script for running LLM integration tests
#
# Usage:
#   ./test_llm.sh [test_filter]
#
# Examples:
#   ./test_llm.sh                    # Run all client tests
#   ./test_llm.sh test_send_message  # Run specific test

if [ $# -eq 0 ]; then
    # Run all client tests
    exec ./with_python_env.sh cargo test --lib client_test -- --nocapture
else
    # Run filtered tests
    exec ./with_python_env.sh cargo test --lib client_test::tests::"$1" -- --nocapture
fi
