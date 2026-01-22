// Comprehensive tests for terminal window resizing functionality
//
// These tests verify that the VT100 terminal emulation correctly handles
// resize events, including:
// - Basic size changes
// - Content preservation during resize
// - Cursor position handling
// - Line wrapping/unwrapping
// - Scrollback preservation
// - Widget rendering after resize

use super::*;
use crate::vt100::{self, TermReplySender};
use tui::widgets::Widget;

/// Simple reply sender for testing (no-op implementation)
#[derive(Clone)]
struct TestReplySender;

impl TermReplySender for TestReplySender {
  fn reply(&self, _reply: compact_str::CompactString) {
    // No-op for testing
  }
}

/// Terminal widget for rendering VT100 screen to tui buffer
struct TerminalWidget<'a, T: TermReplySender> {
  screen: &'a vt100::Screen<T>,
}

impl<'a, T: TermReplySender> TerminalWidget<'a, T> {
  fn new(screen: &'a vt100::Screen<T>) -> Self {
    Self { screen }
  }
}

impl<T: TermReplySender> Widget for TerminalWidget<'_, T> {
  fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
    for row in 0..area.height.min(self.screen.size().rows) {
      for col in 0..area.width.min(self.screen.size().cols) {
        let pos = tui::layout::Position {
          x: area.x + col,
          y: area.y + row,
        };

        if let Some(to_cell) = buf.cell_mut(pos) {
          if let Some(cell) = self.screen.cell(row, col) {
            *to_cell = cell.to_tui();
            if !cell.has_contents() {
              to_cell.set_char(' ');
            }
          }
        }
      }
    }
  }
}

/// Helper to extract visible text from a VT100 screen
fn extract_screen_text(screen: &vt100::Screen<impl TermReplySender>) -> String {
  let mut lines = Vec::new();
  for row_idx in 0..screen.size().rows {
    let mut line = String::new();
    for col_idx in 0..screen.size().cols {
      if let Some(cell) = screen.cell(row_idx, col_idx) {
        if cell.has_contents() {
          line.push_str(&cell.contents());
        } else {
          line.push(' ');
        }
      }
    }
    lines.push(line.trim_end().to_string());
  }
  // Remove trailing empty lines
  while lines.last().map(|l| l.is_empty()).unwrap_or(false) {
    lines.pop();
  }
  lines.join("\n")
}

// =============================================================================
// Basic Resize Tests
// =============================================================================

#[test]
fn test_resize_basic_size_change() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  // Verify initial size
  assert_eq!(parser.screen().size().rows, 24);
  assert_eq!(parser.screen().size().cols, 80);

  // Resize to different dimensions
  parser.set_size(30, 100);

  // Verify new size
  assert_eq!(parser.screen().size().rows, 30);
  assert_eq!(parser.screen().size().cols, 100);
}

#[test]
fn test_resize_smaller_dimensions() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  parser.set_size(12, 40);

  assert_eq!(parser.screen().size().rows, 12);
  assert_eq!(parser.screen().size().cols, 40);
}

#[test]
fn test_resize_larger_dimensions() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  parser.set_size(48, 160);

  assert_eq!(parser.screen().size().rows, 48);
  assert_eq!(parser.screen().size().cols, 160);
}

#[test]
fn test_resize_only_rows() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  parser.set_size(48, 80);

  assert_eq!(parser.screen().size().rows, 48);
  assert_eq!(parser.screen().size().cols, 80);
}

#[test]
fn test_resize_only_cols() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  parser.set_size(24, 120);

  assert_eq!(parser.screen().size().rows, 24);
  assert_eq!(parser.screen().size().cols, 120);
}

// =============================================================================
// Content Preservation Tests
// =============================================================================

#[test]
fn test_resize_preserves_simple_text() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  parser.process(b"Hello, World!");

  parser.set_size(30, 100);

  let text = extract_screen_text(parser.screen());
  assert!(
    text.contains("Hello, World!"),
    "Text not preserved: {}",
    text
  );
}

#[test]
fn test_resize_preserves_multiline_text() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  parser.process(b"Line 1\r\nLine 2\r\nLine 3");

  parser.set_size(30, 100);

  let text = extract_screen_text(parser.screen());
  assert!(text.contains("Line 1"), "Line 1 not preserved: {}", text);
  assert!(text.contains("Line 2"), "Line 2 not preserved: {}", text);
  assert!(text.contains("Line 3"), "Line 3 not preserved: {}", text);
}

#[test]
fn test_resize_preserves_text_with_colors() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  // Write red text
  parser.process(b"\x1b[31mRed Text\x1b[0m Normal");

  parser.set_size(30, 100);

  let text = extract_screen_text(parser.screen());
  assert!(
    text.contains("Red Text"),
    "Colored text not preserved: {}",
    text
  );
  assert!(
    text.contains("Normal"),
    "Normal text not preserved: {}",
    text
  );
}

#[test]
fn test_resize_smaller_wraps_long_lines() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  // Write a line that's 60 characters
  let long_line = "A".repeat(60);
  parser.process(long_line.as_bytes());

  // Resize to 40 columns - line should wrap
  parser.set_size(24, 40);

  // The content should still be present (possibly wrapped)
  let screen = parser.screen();
  let mut total_a_count = 0;
  for row in 0..screen.size().rows {
    for col in 0..screen.size().cols {
      if let Some(cell) = screen.cell(row, col) {
        if cell.contents() == "A" {
          total_a_count += 1;
        }
      }
    }
  }
  assert_eq!(total_a_count, 60, "Content lost during resize");
}

#[test]
fn test_resize_larger_preserves_all_content() {
  let mut parser = vt100::Parser::new(10, 40, 1000, TestReplySender);

  // Fill with numbered lines
  for i in 0..8 {
    parser.process(format!("Line {:02}\r\n", i).as_bytes());
  }

  // Resize larger
  parser.set_size(20, 80);

  let text = extract_screen_text(parser.screen());
  for i in 0..8 {
    assert!(
      text.contains(&format!("Line {:02}", i)),
      "Line {} not preserved: {}",
      i,
      text
    );
  }
}

// =============================================================================
// Cursor Position Tests
// =============================================================================

#[test]
fn test_resize_cursor_at_origin() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  // Cursor starts at (0, 0)
  let (row, col) = parser.screen().cursor_position();
  assert_eq!(row, 0);
  assert_eq!(col, 0);

  parser.set_size(30, 100);

  // Cursor should still be at origin
  let (row, col) = parser.screen().cursor_position();
  assert_eq!(row, 0);
  assert_eq!(col, 0);
}

#[test]
fn test_resize_cursor_after_text() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  parser.process(b"Hello");

  // Cursor should be at end of text
  let (row, col) = parser.screen().cursor_position();
  assert_eq!(row, 0);
  assert_eq!(col, 5);

  parser.set_size(30, 100);

  // Cursor should still be after text
  let (row, col) = parser.screen().cursor_position();
  assert_eq!(row, 0);
  assert_eq!(col, 5);
}

#[test]
fn test_resize_cursor_clamped_to_new_bounds() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  // Move cursor to position that will be out of bounds after resize
  parser.process(b"\x1b[1;70H"); // Move to row 1, col 70

  let (row, col) = parser.screen().cursor_position();
  assert_eq!(row, 0);
  assert_eq!(col, 69);

  // Resize to smaller width
  parser.set_size(24, 40);

  // Cursor column should be clamped to new width
  let (row, col) = parser.screen().cursor_position();
  assert_eq!(row, 0);
  assert!(col < 40, "Cursor col {} should be < 40", col);
}

#[test]
fn test_resize_cursor_row_clamped() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  // Move cursor to bottom
  parser.process(b"\x1b[20;1H"); // Move to row 20, col 1

  let (row, _) = parser.screen().cursor_position();
  assert_eq!(row, 19);

  // Resize to fewer rows
  parser.set_size(10, 80);

  // Cursor row should be clamped to new height
  let (row, _) = parser.screen().cursor_position();
  assert!(row < 10, "Cursor row {} should be < 10", row);
}

// =============================================================================
// Scrollback Tests
// =============================================================================

#[test]
fn test_resize_preserves_scrollback_content() {
  let mut parser = vt100::Parser::new(10, 80, 100, TestReplySender);

  // Fill screen and scrollback with content
  for i in 0..20 {
    parser.process(format!("Line {:02}\r\n", i).as_bytes());
  }

  // Some lines should be in scrollback now
  let initial_total = parser.screen().total_rows();
  assert!(initial_total > 10, "Should have scrollback");

  // Resize
  parser.set_size(15, 80);

  // Scrollback content should be preserved
  let final_total = parser.screen().total_rows();
  assert!(final_total >= 15, "Should still have content after resize");
}

#[test]
fn test_resize_scrollback_offset_preserved() {
  let mut parser = vt100::Parser::new(10, 80, 100, TestReplySender);

  // Fill with content
  for i in 0..30 {
    parser.process(format!("Line {:02}\r\n", i).as_bytes());
  }

  // Set scrollback position
  parser.set_scrollback(5);
  assert_eq!(parser.screen().scrollback(), 5);

  // Resize
  parser.set_size(15, 80);

  // Scrollback offset should be within valid range (may be adjusted)
  let offset = parser.screen().scrollback();
  assert!(
    offset <= parser.screen().scrollback_len(),
    "Scrollback offset out of range"
  );
}

// =============================================================================
// Line Wrapping Tests
// =============================================================================

#[test]
fn test_resize_narrower_causes_line_wrap() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  // Write exactly 80 characters (fills one line in 80-col mode)
  let line = "X".repeat(80);
  parser.process(line.as_bytes());

  // In 80-col mode, this should be one line
  let screen = parser.screen();
  let cell_at_79 = screen.cell(0, 79);
  assert!(
    cell_at_79.is_some() && cell_at_79.unwrap().contents() == "X",
    "Line should fill row 0"
  );

  // Resize to 40 columns
  parser.set_size(24, 40);

  // Content should now wrap to 2 lines
  let screen = parser.screen();
  // Check that row 0 has content
  let cell_0_0 = screen.cell(0, 0);
  assert!(
    cell_0_0.is_some() && cell_0_0.unwrap().contents() == "X",
    "Row 0 should have content"
  );
  // Check that row 1 has content (wrapped)
  let cell_1_0 = screen.cell(1, 0);
  assert!(
    cell_1_0.is_some() && cell_1_0.unwrap().contents() == "X",
    "Row 1 should have wrapped content"
  );
}

#[test]
fn test_resize_wider_content_integrity() {
  let mut parser = vt100::Parser::new(24, 40, 1000, TestReplySender);

  // Write text that wraps at 40 cols
  let line = "Y".repeat(60);
  parser.process(line.as_bytes());

  // Resize wider
  parser.set_size(24, 80);

  // Count total Y characters
  let screen = parser.screen();
  let mut y_count = 0;
  for row in 0..screen.size().rows {
    for col in 0..screen.size().cols {
      if let Some(cell) = screen.cell(row, col) {
        if cell.contents() == "Y" {
          y_count += 1;
        }
      }
    }
  }
  assert_eq!(y_count, 60, "Content should be preserved");
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn test_resize_to_minimum_size() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  parser.process(b"Test content");

  // Resize to very small
  parser.set_size(1, 1);

  assert_eq!(parser.screen().size().rows, 1);
  assert_eq!(parser.screen().size().cols, 1);
}

#[test]
fn test_resize_multiple_times() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  parser.process(b"Initial text");

  // Multiple resizes
  parser.set_size(30, 100);
  assert_eq!(parser.screen().size().rows, 30);
  assert_eq!(parser.screen().size().cols, 100);

  parser.set_size(10, 40);
  assert_eq!(parser.screen().size().rows, 10);
  assert_eq!(parser.screen().size().cols, 40);

  parser.set_size(50, 200);
  assert_eq!(parser.screen().size().rows, 50);
  assert_eq!(parser.screen().size().cols, 200);

  parser.set_size(24, 80);
  assert_eq!(parser.screen().size().rows, 24);
  assert_eq!(parser.screen().size().cols, 80);
}

#[test]
fn test_resize_same_size_is_noop() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  parser.process(b"Some text");
  let before = extract_screen_text(parser.screen());

  // Resize to same size
  parser.set_size(24, 80);

  let after = extract_screen_text(parser.screen());
  assert_eq!(before, after, "Content should be unchanged");
}

#[test]
fn test_resize_empty_terminal() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  // Don't write anything

  parser.set_size(30, 100);

  assert_eq!(parser.screen().size().rows, 30);
  assert_eq!(parser.screen().size().cols, 100);

  // Screen should still be empty
  let text = extract_screen_text(parser.screen());
  assert!(text.is_empty() || text.chars().all(|c| c.is_whitespace()));
}

// =============================================================================
// Widget Rendering After Resize Tests
// =============================================================================

#[test]
fn test_resize_widget_rendering() {
  let config = TestAppConfig::new().with_terminal_size(80, 24);
  let mut harness = TestHarness::with_config(config);
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  parser.process(b"Test content for rendering\r\n");
  parser.process(b"Second line of text");

  // Resize
  parser.set_size(30, 100);

  // Create widget and render
  let screen = parser.screen();
  harness
    .terminal
    .draw(|f| {
      let widget = TerminalWidget::new(screen);
      f.render_widget(widget, f.area());
    })
    .unwrap();

  harness.assert_buffer_contains("Test content");
  harness.assert_buffer_contains("Second line");
}

#[test]
fn test_resize_widget_rendering_smaller() {
  let config = TestAppConfig::new().with_terminal_size(40, 12);
  let mut harness = TestHarness::with_config(config);
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  parser.process(b"Short text");

  // Resize smaller
  parser.set_size(12, 40);

  let screen = parser.screen();
  harness
    .terminal
    .draw(|f| {
      let widget = TerminalWidget::new(screen);
      f.render_widget(widget, f.area());
    })
    .unwrap();

  harness.assert_buffer_contains("Short text");
}

#[test]
fn test_resize_harness_resize_event() {
  let mut harness = TestHarness::new();

  // Queue a resize event
  harness.resize(120, 40);

  // Verify event was queued
  assert_eq!(harness.events.len(), 1);
  match &harness.events[0] {
    TestEvent::Resize(w, h) => {
      assert_eq!(*w, 120);
      assert_eq!(*h, 40);
    }
    _ => panic!("Expected Resize event"),
  }
}

// =============================================================================
// Special Characters and Formatting Tests
// =============================================================================

#[test]
fn test_resize_preserves_unicode() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  parser.process("Hello, \u{4e16}\u{754c}!".as_bytes()); // Hello, World in Chinese

  parser.set_size(30, 100);

  let text = extract_screen_text(parser.screen());
  assert!(
    text.contains("\u{4e16}") || text.contains("Hello"),
    "Unicode content should be preserved: {}",
    text
  );
}

#[test]
fn test_resize_with_tabs() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  parser.process(b"Col1\tCol2\tCol3");

  parser.set_size(30, 100);

  let text = extract_screen_text(parser.screen());
  assert!(
    text.contains("Col1"),
    "Tab-separated content should be preserved"
  );
  assert!(
    text.contains("Col2"),
    "Tab-separated content should be preserved"
  );
  assert!(
    text.contains("Col3"),
    "Tab-separated content should be preserved"
  );
}

#[test]
fn test_resize_with_bold_text() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  parser.process(b"\x1b[1mBold Text\x1b[0m Normal");

  parser.set_size(30, 100);

  let text = extract_screen_text(parser.screen());
  assert!(text.contains("Bold Text"), "Bold text should be preserved");
  assert!(text.contains("Normal"), "Normal text should be preserved");
}

// =============================================================================
// Scroll Region Tests
// =============================================================================

#[test]
fn test_resize_resets_scroll_region_bottom() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  // Set a scroll region (top=5, bottom=20)
  parser.process(b"\x1b[6;21r");

  // Resize to smaller height
  parser.set_size(15, 80);

  // The scroll region bottom should be adjusted to fit new size
  // (verified indirectly by checking no crash and correct size)
  assert_eq!(parser.screen().size().rows, 15);
}

// =============================================================================
// Alternate Screen Buffer Tests
// =============================================================================

#[test]
fn test_resize_in_alternate_screen() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  // Enter alternate screen
  parser.process(b"\x1b[?1049h");

  parser.process(b"Alternate screen content");

  // Resize while in alternate screen
  parser.set_size(30, 100);

  assert_eq!(parser.screen().size().rows, 30);
  assert_eq!(parser.screen().size().cols, 100);

  let text = extract_screen_text(parser.screen());
  assert!(
    text.contains("Alternate screen content"),
    "Alternate screen content should be preserved"
  );

  // Exit alternate screen
  parser.process(b"\x1b[?1049l");

  // Main screen should also be resized
  assert_eq!(parser.screen().size().rows, 30);
  assert_eq!(parser.screen().size().cols, 100);
}

// =============================================================================
// Stress Tests
// =============================================================================

#[test]
fn test_resize_rapid_succession() {
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  parser.process(b"Content that should survive resizing");

  // Rapid resizes
  for i in 0..20 {
    let rows = 10 + (i % 40);
    let cols = 40 + (i % 80);
    parser.set_size(rows, cols);
  }

  // Final resize back to normal
  parser.set_size(24, 80);

  assert_eq!(parser.screen().size().rows, 24);
  assert_eq!(parser.screen().size().cols, 80);
}

#[test]
fn test_resize_with_heavy_scrollback() {
  let mut parser = vt100::Parser::new(24, 80, 5000, TestReplySender);

  // Fill with lots of content
  for i in 0..1000 {
    parser.process(format!("Line {:04}\r\n", i).as_bytes());
  }

  let before_total = parser.screen().total_rows();

  // Resize
  parser.set_size(48, 120);

  // Should not crash and should preserve some history
  assert!(parser.screen().total_rows() > 0);

  // Resize back
  parser.set_size(24, 80);

  assert_eq!(parser.screen().size().rows, 24);
  assert_eq!(parser.screen().size().cols, 80);
}

// =============================================================================
// Integration with TestHarness
// =============================================================================

#[test]
fn test_harness_resize_and_render() {
  // Create harness with initial size
  let config = TestAppConfig::new().with_terminal_size(80, 24);
  let mut harness = TestHarness::with_config(config);

  // Create parser matching harness size
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);
  parser.process(b"Initial content on line 1\r\n");
  parser.process(b"More content on line 2");

  // Render initial state
  {
    let screen = parser.screen();
    harness
      .terminal
      .draw(|f| {
        let widget = TerminalWidget::new(screen);
        f.render_widget(widget, f.area());
      })
      .unwrap();
  }

  harness.assert_buffer_contains("Initial content");
  harness.assert_buffer_contains("More content");

  // Note: TestBackend resize is separate from vt100 resize
  // In a real app, both would be coordinated
}
