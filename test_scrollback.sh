#!/bin/bash
# Test script to verify native scrollback in terminai
# This script outputs many lines to test if they scroll into the host terminal's scrollback

echo "=== Native Scrollback Test ==="
echo "This test will output 100 lines."
echo "After it finishes, try scrolling up in your terminal with your mouse wheel or touchpad."
echo "You should be able to see all 100 lines in your terminal's native scrollback."
echo ""
echo "Press Enter to start..."
read

for i in {1..100}; do
  echo "Line $i: This is test content to verify scrollback functionality"
  sleep 0.05
done

echo ""
echo "=== Test Complete ==="
echo "Try scrolling up now with your mouse wheel."
echo "You should see all 100 lines in your terminal's scrollback buffer."
