//! Shadow terminal that mirrors the guest terminal state
//!
//! This module provides a wrapper around the vt100 parser that maintains a
//! parallel terminal state. It's used for ratatui rendering when the modal
//! is visible or when the guest is in alternate buffer mode.

use crate::vt100::{Parser, Screen, TermReplySender};

use super::content::{TerminalContent, map_cell};

/// Events that can be detected from terminal output
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEvent {
  /// Guest terminal switched alternate buffer state
  AltBufferChange(bool),

  /// Terminal title changed
  TitleChange(String),

  /// Bell was rung
  Bell,
}

/// Shadow terminal that mirrors the guest terminal state
///
/// This wraps the mprocs vt100 parser to maintain a parallel terminal state
/// that can be used for ratatui rendering.
pub struct ShadowTerminal<Reply: TermReplySender + Clone> {
  /// The vt100 parser from mprocs
  parser: Parser<Reply>,

  /// Terminal dimensions
  size: (u16, u16), // (cols, rows)

  /// Last known alternate buffer state
  in_alt_buffer: bool,
}

impl<Reply: TermReplySender + Clone> ShadowTerminal<Reply> {
  /// Create a new shadow terminal with the given dimensions
  pub fn new(
    cols: u16,
    rows: u16,
    scrollback_lines: usize,
    reply_sender: Reply,
  ) -> Self {
    Self {
      parser: Parser::new(rows, cols, scrollback_lines, reply_sender),
      size: (cols, rows),
      in_alt_buffer: false,
    }
  }

  /// Process terminal output, updating internal state
  ///
  /// Returns any detected control events (like alt buffer switches)
  pub fn process(&mut self, data: &[u8]) -> Vec<TerminalEvent> {
    let mut events = Vec::new();

    // Track alt buffer state before processing
    let was_in_alt = self.in_alt_buffer;

    // Process through vt100 parser
    self.parser.process(data);

    // Detect alt buffer change by checking if the current grid changed
    // The vt100 parser switches between main_grid and alternate_grid internally
    // We can detect this by checking the cursor position change pattern
    // For now, we'll use a simpler approach: check if we see the escape sequences
    let now_in_alt = self.detect_alt_buffer_state(data);
    if now_in_alt != self.in_alt_buffer {
      self.in_alt_buffer = now_in_alt;
    }

    // Check for alt buffer transition
    if was_in_alt != self.in_alt_buffer {
      events.push(TerminalEvent::AltBufferChange(self.in_alt_buffer));
    }

    // Check for bell
    if data.contains(&0x07) {
      events.push(TerminalEvent::Bell);
    }

    events
  }

  /// Detect if we're in alternate buffer by looking for escape sequences
  ///
  /// This is a simple heuristic that looks for the common alt buffer sequences:
  /// - CSI ?1049h - Enter alt buffer (xterm)
  /// - CSI ?1049l - Leave alt buffer (xterm)
  /// - CSI ?47h - Enter alt buffer (older)
  /// - CSI ?47l - Leave alt buffer (older)
  /// - CSI ?1047h - Enter alt buffer (screen)
  /// - CSI ?1047l - Leave alt buffer (screen)
  fn detect_alt_buffer_state(&self, data: &[u8]) -> bool {
    let data_str = String::from_utf8_lossy(data);

    // Check for enter alt buffer sequences
    if data_str.contains("\x1b[?1049h")
      || data_str.contains("\x1b[?47h")
      || data_str.contains("\x1b[?1047h")
    {
      return true;
    }

    // Check for leave alt buffer sequences
    if data_str.contains("\x1b[?1049l")
      || data_str.contains("\x1b[?47l")
      || data_str.contains("\x1b[?1047l")
    {
      return false;
    }

    // No change detected, return current state
    self.in_alt_buffer
  }

  /// Get whether guest is in alternate buffer
  pub fn is_in_alt_buffer(&self) -> bool {
    self.in_alt_buffer
  }

  /// Resize the terminal
  pub fn resize(&mut self, cols: u16, rows: u16) {
    self.parser.set_size(rows, cols);
    self.size = (cols, rows);
  }

  /// Get the terminal dimensions
  pub fn size(&self) -> (u16, u16) {
    self.size
  }

  /// Get visible content as a grid suitable for ratatui
  pub fn visible_content(&self) -> TerminalContent {
    let screen = self.parser.screen();
    let (cols, rows) = self.size;

    let cells = (0..rows)
      .map(|row| {
        (0..cols)
          .map(|col| map_cell(screen.cell(row, col)))
          .collect()
      })
      .collect();

    let cursor = screen.cursor_position();
    let cursor_visible = !screen.hide_cursor();

    TerminalContent {
      cells,
      cursor,
      cursor_visible,
      size: self.size,
    }
  }

  /// Get a reference to the underlying screen
  pub fn screen(&self) -> &Screen<Reply> {
    self.parser.screen()
  }

  /// Get the current cursor position
  pub fn cursor_position(&self) -> (u16, u16) {
    self.parser.screen().cursor_position()
  }

  /// Check if cursor is visible
  pub fn cursor_visible(&self) -> bool {
    !self.parser.screen().hide_cursor()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::vt100::TermReplySender;

  #[derive(Clone, Debug)]
  struct NoOpReplySender;

  impl TermReplySender for NoOpReplySender {
    fn send(&self, _data: Vec<u8>) {}
  }

  #[test]
  fn test_create_shadow_terminal() {
    let shadow = ShadowTerminal::new(80, 24, 1000, NoOpReplySender);
    assert_eq!(shadow.size(), (80, 24));
    assert!(!shadow.is_in_alt_buffer());
  }

  #[test]
  fn test_process_simple_text() {
    let mut shadow = ShadowTerminal::new(80, 24, 1000, NoOpReplySender);
    let events = shadow.process(b"Hello, World!");
    assert!(events.is_empty());

    let content = shadow.visible_content();
    assert_eq!(content.size, (80, 24));
  }

  #[test]
  fn test_alt_buffer_detection() {
    let mut shadow = ShadowTerminal::new(80, 24, 1000, NoOpReplySender);

    // Enter alt buffer
    let events = shadow.process(b"\x1b[?1049h");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0], TerminalEvent::AltBufferChange(true));
    assert!(shadow.is_in_alt_buffer());

    // Leave alt buffer
    let events = shadow.process(b"\x1b[?1049l");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0], TerminalEvent::AltBufferChange(false));
    assert!(!shadow.is_in_alt_buffer());
  }

  #[test]
  fn test_bell_detection() {
    let mut shadow = ShadowTerminal::new(80, 24, 1000, NoOpReplySender);
    let events = shadow.process(b"Hello\x07World");

    // Should detect bell
    assert!(events.iter().any(|e| matches!(e, TerminalEvent::Bell)));
  }

  #[test]
  fn test_resize() {
    let mut shadow = ShadowTerminal::new(80, 24, 1000, NoOpReplySender);
    shadow.resize(100, 30);
    assert_eq!(shadow.size(), (100, 30));

    let content = shadow.visible_content();
    assert_eq!(content.size, (100, 30));
    assert_eq!(content.cells.len(), 30);
    assert_eq!(content.cells[0].len(), 100);
  }

  #[test]
  fn test_cursor_tracking() {
    let mut shadow = ShadowTerminal::new(80, 24, 1000, NoOpReplySender);

    // Initial cursor should be at (0, 0)
    let (row, col) = shadow.cursor_position();
    assert_eq!((row, col), (0, 0));
    assert!(shadow.cursor_visible());

    // Write some text and check cursor moved
    shadow.process(b"Hello");
    let (row, col) = shadow.cursor_position();
    assert_eq!((row, col), (0, 5));
  }

  #[test]
  fn test_visible_content_extraction() {
    let mut shadow = ShadowTerminal::new(80, 24, 1000, NoOpReplySender);
    shadow.process(b"Test");

    let content = shadow.visible_content();
    assert_eq!(content.width(), 80);
    assert_eq!(content.height(), 24);
    assert!(content.cursor_visible);

    // Check that first few cells contain our text
    assert_eq!(content.cells[0][0].symbol(), "T");
    assert_eq!(content.cells[0][1].symbol(), "e");
    assert_eq!(content.cells[0][2].symbol(), "s");
    assert_eq!(content.cells[0][3].symbol(), "t");
  }
}
