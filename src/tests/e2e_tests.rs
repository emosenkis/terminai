// E2E tests for Termin.AI
//
// These tests use ratatui's TestBackend to verify the application's
// behavior without requiring an actual terminal.

use super::e2e::TestHarness;

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_harness_smoke_test() {
    // Verify the test harness itself works
    let harness = TestHarness::new();
    assert_eq!(harness.size(), (80, 24));
  }
}
