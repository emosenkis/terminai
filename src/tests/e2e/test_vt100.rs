// VT100 terminal emulation tests
//
// Tests for VT100 terminal parsing and rendering

use super::*;
use crate::ui_layer::TerminalWidget;
use crate::vt100::Color;
use crate::vt100::{self, TermReplySender};
use std::sync::{Arc, Mutex};
use tui::widgets::Widget;

/// Simple reply sender for testing
#[derive(Clone)]
struct TestReplySender;

impl TermReplySender for TestReplySender {
  fn reply(&self, _reply: compact_str::CompactString) {
    // No-op for testing
  }
}

#[derive(Clone)]
struct HostEscapeReplySender {
  host_escapes: Arc<Mutex<Vec<String>>>,
}

impl TermReplySender for HostEscapeReplySender {
  fn reply(&self, _reply: compact_str::CompactString) {}

  fn host_escape(&self, escape: compact_str::CompactString) {
    self.host_escapes.lock().unwrap().push(escape.to_string());
  }
}

#[derive(Clone)]
struct CapturingReplySender {
  replies: Arc<Mutex<Vec<String>>>,
}

impl TermReplySender for CapturingReplySender {
  fn reply(&self, reply: compact_str::CompactString) {
    self.replies.lock().unwrap().push(reply.to_string());
  }
}

#[test]
fn test_vt100_replies_to_terminal_status_dsr() {
  let replies = Arc::new(Mutex::new(Vec::new()));
  let mut parser = vt100::Parser::new(
    24,
    80,
    1000,
    CapturingReplySender {
      replies: replies.clone(),
    },
  );

  parser.process(b"\x1b[5n");

  assert_eq!(replies.lock().unwrap().as_slice(), ["\x1b[0n"]);
}

#[test]
fn test_vt100_replies_to_cursor_position_dsr() {
  let replies = Arc::new(Mutex::new(Vec::new()));
  let mut parser = vt100::Parser::new(
    24,
    80,
    1000,
    CapturingReplySender {
      replies: replies.clone(),
    },
  );

  parser.process(b"\x1b[2;3H\x1b[6n");

  assert_eq!(replies.lock().unwrap().as_slice(), ["\x1b[2;3R"]);
}

#[test]
fn test_vt100_replies_to_dec_private_cursor_position_dsr() {
  let replies = Arc::new(Mutex::new(Vec::new()));
  let mut parser = vt100::Parser::new(
    24,
    80,
    1000,
    CapturingReplySender {
      replies: replies.clone(),
    },
  );

  parser.process(b"\x1b[4;5H\x1b[?6n");

  assert_eq!(replies.lock().unwrap().as_slice(), ["\x1b[?4;5R"]);
}

#[test]
fn test_vt100_forwards_current_working_directory_osc() {
  let host_escapes = Arc::new(Mutex::new(Vec::new()));
  let mut parser = vt100::Parser::new(
    24,
    80,
    1000,
    HostEscapeReplySender {
      host_escapes: host_escapes.clone(),
    },
  );

  parser.process(b"\x1b]7;file://host/tmp/project\x07");

  assert_eq!(
    host_escapes.lock().unwrap().as_slice(),
    ["\x1b]7;file://host/tmp/project\x07"]
  );
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
  let cell = screen.cell(0, 0).expect("red text cell should exist");
  assert_eq!(cell.fgcolor(), Color::Idx(1));
}

#[test]
fn test_vt100_preserves_256_color_palette_indices() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  parser.process(b"\x1b[38;5;123mF\x1b[48;5;45mB");

  let screen = parser.screen();
  let fg = screen.cell(0, 0).expect("foreground cell should exist");
  let bg = screen.cell(0, 1).expect("background cell should exist");
  assert_eq!(fg.fgcolor(), Color::Idx(123));
  assert_eq!(bg.bgcolor(), Color::Idx(45));
}

#[test]
fn test_vt100_preserves_truecolor_as_rgb() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  parser.process(b"\x1b[38;2;12;34;56mT");

  let cell = parser
    .screen()
    .cell(0, 0)
    .expect("truecolor cell should exist");
  assert_eq!(cell.fgcolor(), Color::Rgb(12, 34, 56));
}

#[test]
fn test_vt100_preserves_indexed_underline_color() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  parser.process(b"\x1b[4;58;5;201mU");

  let cell = parser
    .screen()
    .cell(0, 0)
    .expect("underline color cell should exist");
  assert_eq!(cell.underline_color(), Color::Idx(201));
  assert!(cell.underline());
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
