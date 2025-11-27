//! Host terminal controller for managing the actual terminal state
//!
//! This module provides control over the host terminal, including entering/leaving
//! alternate buffer and writing raw output.

use crossterm::{
  execute,
  terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self, Stdout, Write};

/// Controls the host terminal state
///
/// This component manages the actual terminal (stdout) and tracks whether
/// we're in the alternate buffer.
pub struct HostTerminalController {
  /// The stdout handle
  stdout: Stdout,

  /// Whether we're currently in alternate buffer
  in_alt_buffer: bool,
}

impl HostTerminalController {
  /// Create a new host terminal controller
  pub fn new() -> Self {
    Self {
      stdout: io::stdout(),
      in_alt_buffer: false,
    }
  }

  /// Enter the alternate buffer
  ///
  /// This is idempotent - calling it multiple times is safe.
  pub fn enter_alt_buffer(&mut self) -> io::Result<()> {
    if !self.in_alt_buffer {
      execute!(self.stdout, EnterAlternateScreen)?;
      self.in_alt_buffer = true;
    }
    Ok(())
  }

  /// Leave the alternate buffer
  ///
  /// This is idempotent - calling it multiple times is safe.
  pub fn leave_alt_buffer(&mut self) -> io::Result<()> {
    if self.in_alt_buffer {
      execute!(self.stdout, LeaveAlternateScreen)?;
      self.in_alt_buffer = false;
    }
    Ok(())
  }

  /// Write raw bytes to stdout
  ///
  /// This is used for passthrough mode where we're directly writing
  /// guest terminal output to the host.
  pub fn write_raw(&mut self, data: &[u8]) -> io::Result<()> {
    self.stdout.write_all(data)?;
    self.stdout.flush()?;
    Ok(())
  }

  /// Check if we're currently in alternate buffer
  pub fn is_in_alt_buffer(&self) -> bool {
    self.in_alt_buffer
  }

  /// Flush any pending output
  pub fn flush(&mut self) -> io::Result<()> {
    self.stdout.flush()
  }

  /// Get a mutable reference to stdout
  ///
  /// This can be used for operations that need direct access to stdout,
  /// like ratatui rendering.
  pub fn stdout_mut(&mut self) -> &mut Stdout {
    &mut self.stdout
  }
}

impl Default for HostTerminalController {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  // Note: These tests are limited because they interact with the actual terminal.
  // In a real implementation, you might want to use a mock stdout for testing.

  #[test]
  fn test_initial_state() {
    let controller = HostTerminalController::new();
    assert!(!controller.is_in_alt_buffer());
  }

  // We can't easily test the actual enter/leave alt buffer functionality
  // without mocking the terminal, so we just verify the API exists
  #[test]
  fn test_idempotent_enter() {
    let mut controller = HostTerminalController::new();

    // These operations should be safe to call multiple times
    // In a real terminal, the second call would be a no-op
    let result1 = controller.enter_alt_buffer();
    let result2 = controller.enter_alt_buffer();

    // We can't assert success here as we might not have a real terminal
    // in the test environment, but we can verify they don't panic
    drop(result1);
    drop(result2);
  }

  #[test]
  fn test_idempotent_leave() {
    let mut controller = HostTerminalController::new();

    // Should be safe to leave even if not entered
    let result1 = controller.leave_alt_buffer();
    let result2 = controller.leave_alt_buffer();

    drop(result1);
    drop(result2);
  }
}
