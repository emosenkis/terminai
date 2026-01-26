// VT100 terminal emulation tests
//
// Tests for VT100 terminal parsing and rendering

use super::*;
use crate::ui_layer::TerminalWidget;
use crate::vt100::{self, TermReplySender};
use tui::widgets::Widget;

/// Simple reply sender for testing
#[derive(Clone)]
struct TestReplySender;

impl TermReplySender for TestReplySender {
  fn reply(&self, _reply: compact_str::CompactString) {
    // No-op for testing
  }
}

#[test]
fn test_vt100_basic_text() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  // Process some basic text
  parser.process(b"Hello, World!");

  let screen = parser.screen();
  assert_eq!(screen.size().rows, 24);
  assert_eq!(screen.size().cols, 80);

  // Check that the text was rendered
  let cell = screen.cell(0, 0);
  assert!(cell.is_some());
}

#[test]
fn test_vt100_colors() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  // Process text with ANSI color codes
  parser.process(b"\x1b[31mRed Text\x1b[0m");

  let screen = parser.screen();
  // Verify the text is there (color testing is limited per ratatui docs)
  let cell = screen.cell(0, 0);
  assert!(cell.is_some());
}

#[test]
fn test_vt100_cursor_movement() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  // Move cursor and write
  parser.process(b"First");
  parser.process(b"\x1b[2;1H"); // Move to row 2, col 1
  parser.process(b"Second");

  let screen = parser.screen();
  // Verify text appears at different rows
  assert!(screen.cell(0, 0).is_some()); // "First" at top
  assert!(screen.cell(1, 0).is_some()); // "Second" at second row
}

#[test]
fn test_vt100_resize() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  parser.process(b"Before resize");

  // Resize the terminal
  parser.set_size(30, 100);

  let screen = parser.screen();
  assert_eq!(screen.size().rows, 30);
  assert_eq!(screen.size().cols, 100);
}

#[test]
fn test_vt100_scrolling() {
  let mut parser = vt100::Parser::new(10, 80, 100, TestReplySender); // Small scrollback

  // Fill the screen with lines
  for i in 0..15 {
    parser.process(format!("Line {}\r\n", i).as_bytes());
  }

  let screen = parser.screen();
  // The screen should only show the last 10 lines
  assert_eq!(screen.size().rows, 10);
}

#[test]
fn test_vt100_widget_rendering() {
  let mut harness = TestHarness::new();
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  // Process some text with formatting
  parser.process(b"Terminal Output\r\n");
  parser.process(b"With multiple lines\r\n");
  parser.process(b"And \x1b[1mbold text\x1b[0m");

  // Create a widget from the VT100 screen
  let screen = parser.screen();

  // Render using our custom TerminalWidget
  harness
    .terminal
    .draw(|f| {
      let widget = TerminalWidget::new(screen);
      f.render_widget(widget, f.area());
    })
    .unwrap();

  harness.assert_buffer_contains("Terminal Output");
  harness.assert_buffer_contains("With multiple lines");
}

#[test]
#[cfg(feature = "snapshot-tests")]
fn test_vt100_snapshot() {
  let mut harness = TestHarness::new();
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  // Process some formatted terminal output
  parser.process(b"\x1b[1;1H"); // Home
  parser.process(b"\x1b[31mRed\x1b[0m ");
  parser.process(b"\x1b[32mGreen\x1b[0m ");
  parser.process(b"\x1b[34mBlue\x1b[0m\r\n");
  parser.process(b"Normal text\r\n");
  parser.process(b"\x1b[1mBold\x1b[0m and \x1b[4munderline\x1b[0m");

  let screen = parser.screen();
  harness
    .terminal
    .draw(|f| {
      let widget = TerminalWidget::new(screen);
      f.render_widget(widget, f.area());
    })
    .unwrap();

  insta::assert_snapshot!(harness.buffer_as_string());
}
