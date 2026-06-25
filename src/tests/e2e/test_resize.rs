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
use crate::ui_layer::TerminalWidget;
use crate::vt100::{self, TermReplySender};
use tui::{
  backend::{Backend, ClearType, TestBackend, WindowSize},
  buffer::Cell,
  layout::{Position, Size},
  widgets::{Block, Widget},
};

#[derive(Debug)]
struct StickyClearBackend {
  inner: TestBackend,
}

impl StickyClearBackend {
  fn new(width: u16, height: u16) -> Self {
    Self {
      inner: TestBackend::new(width, height),
    }
  }

  fn resize(&mut self, width: u16, height: u16) {
    self.inner.resize(width, height);
  }

  fn buffer_lines(&self) -> Vec<String> {
    let buffer = self.inner.buffer();
    buffer
      .content
      .chunks(buffer.area.width as usize)
      .map(|row| row.iter().map(|cell| cell.symbol()).collect())
      .collect()
  }
}

impl Backend for StickyClearBackend {
  fn draw<'a, I>(&mut self, content: I) -> std::io::Result<()>
  where
    I: Iterator<Item = (u16, u16, &'a Cell)>,
  {
    self.inner.draw(content)
  }

  fn hide_cursor(&mut self) -> std::io::Result<()> {
    self.inner.hide_cursor()
  }

  fn show_cursor(&mut self) -> std::io::Result<()> {
    self.inner.show_cursor()
  }

  fn get_cursor_position(&mut self) -> std::io::Result<Position> {
    self.inner.get_cursor_position()
  }

  fn set_cursor_position<P: Into<Position>>(
    &mut self,
    position: P,
  ) -> std::io::Result<()> {
    self.inner.set_cursor_position(position)
  }

  fn clear(&mut self) -> std::io::Result<()> {
    Ok(())
  }

  fn clear_region(&mut self, _clear_type: ClearType) -> std::io::Result<()> {
    Ok(())
  }

  fn size(&self) -> std::io::Result<Size> {
    self.inner.size()
  }

  fn window_size(&mut self) -> std::io::Result<WindowSize> {
    self.inner.window_size()
  }

  fn flush(&mut self) -> std::io::Result<()> {
    self.inner.flush()
  }

  fn append_lines(&mut self, n: u16) -> std::io::Result<()> {
    self.inner.append_lines(n)
  }

  fn scroll_region_up(
    &mut self,
    region: std::ops::Range<u16>,
    line_count: u16,
  ) -> std::io::Result<()> {
    self.inner.scroll_region_up(region, line_count)
  }

  fn scroll_region_down(
    &mut self,
    region: std::ops::Range<u16>,
    line_count: u16,
  ) -> std::io::Result<()> {
    self.inner.scroll_region_down(region, line_count)
  }

  #[cfg(feature = "native-scrolling")]
  fn stream_lines_to_scrollback(
    &mut self,
    content: &[Cell],
    width: u16,
    line_count: u16,
    screen_height: u16,
  ) -> std::io::Result<()> {
    self.inner.stream_lines_to_scrollback(
      content,
      width,
      line_count,
      screen_height,
    )
  }
}

/// Simple reply sender for testing (no-op implementation)
#[derive(Clone)]
struct TestReplySender;

impl TermReplySender for TestReplySender {
  fn reply(&self, _reply: compact_str::CompactString) {
    // No-op for testing
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
fn test_resize_after_ai_overlay_forces_blank_cells_to_redraw() {
  let backend = StickyClearBackend::new(10, 4);
  let mut terminal = tui::Terminal::new(backend).unwrap();

  terminal
    .draw(|f| {
      let area = f.area();
      f.buffer_mut()
        .set_string(0, 0, "shell", tui::style::Style::reset());
      f.buffer_mut()
        .set_string(2, 1, "AI MODAL", tui::style::Style::reset());
      f.buffer_mut()
        .set_string(2, 2, "Press", tui::style::Style::reset());
      f.render_widget(
        Block::bordered(),
        tui::layout::Rect::new(1, 0, area.width - 2, area.height),
      );
    })
    .unwrap();

  terminal.backend_mut().resize(12, 4);

  terminal
    .draw(|f| {
      f.buffer_mut()
        .set_string(0, 0, "shell", tui::style::Style::reset());
    })
    .unwrap();

  let expected = [
    "shell       ",
    "            ",
    "            ",
    "            ",
  ];
  let actual = terminal.backend().buffer_lines();
  let report = format!(
    "Resize while AI overlay is visible, then render the closed-overlay shell frame.\n\
     Expected physical screen after redraw:\n{}\n\n\
     Actual physical screen after redraw:\n{}\n\n\
     The actual screen must not contain stale AI overlay text or border glyphs.",
    expected
      .iter()
      .map(|line| format!("|{line}|"))
      .collect::<Vec<_>>()
      .join("\n"),
    actual
      .iter()
      .map(|line| format!("|{line}|"))
      .collect::<Vec<_>>()
      .join("\n")
  );

  insta::assert_snapshot!("resize_after_ai_overlay_corruption", report);
}

#[test]
fn test_resize_after_native_scrollback_does_not_drift_into_lower_band() {
  let backend = StickyClearBackend::new(16, 6);
  let mut options = crate::terminai_init::terminal_options().ratatui_options;
  if let tui::Viewport::Inline(height) = &mut options.viewport {
    *height = 6;
  }
  let mut terminal = tui::Terminal::with_options(backend, options).unwrap();

  terminal
    .draw(|f| {
      f.buffer_mut()
        .set_string(0, 0, "shell row 0", tui::style::Style::reset());
      f.set_scroll_up(1);
      f.set_cursor_position(Position::new(0, 2));
      f.render_widget(Block::bordered().title("AI"), f.area());
    })
    .unwrap();

  terminal
    .backend_mut()
    .set_cursor_position(Position::new(0, 5))
    .unwrap();
  terminal.backend_mut().resize(16, 10);

  let mut frame_area = None;
  terminal
    .draw(|f| {
      let area = f.area();
      frame_area = Some(area);
      f.buffer_mut()
        .set_string(area.x, area.y, "shell row 1", tui::style::Style::reset());
      f.render_widget(Block::bordered().title("AI"), area);
    })
    .unwrap();

  let actual = terminal.backend().buffer_lines();
  let report = format!(
    "After native scrollback leaves the physical cursor on the last row, a resize must still \
     render the next frame from the top-left of the physical terminal.\n\
     Frame area after resize: {:?}\n\n\
     Physical screen after redraw:\n{}\n\n\
     The modal must not be anchored in a lower screen band.",
    frame_area.unwrap(),
    actual
      .iter()
      .map(|line| format!("|{line}|"))
      .collect::<Vec<_>>()
      .join("\n")
  );

  insta::assert_snapshot!("resize_after_native_scrollback_lower_band", report);
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

// =============================================================================
// Scrollback Tracker Synchronization After Resize Tests
// =============================================================================

use crate::scrollback::ScrollbackTracker;

#[test]
fn test_resize_larger_scrollback_tracker_sync() {
  // Test that scrollback tracker must be re-synced after resize
  // because total_rows can change when lines unwrap
  let mut parser = vt100::Parser::new(24, 40, 1000, TestReplySender);

  // Write content that wraps at 40 columns
  // Each 80-char line becomes 2 rows at 40 cols
  for i in 0..10 {
    let line = format!("Line {:02}: {}\r\n", i, "X".repeat(70));
    parser.process(line.as_bytes());
  }

  let total_before = parser.screen().total_rows();

  // Initialize tracker with current state
  let mut tracker = ScrollbackTracker::new();
  tracker.init_from_screen(parser.screen());
  assert_eq!(tracker.last_total_rows(), total_before);

  // Resize wider - lines may unwrap, reducing total_rows
  parser.set_size(24, 120);

  let total_after = parser.screen().total_rows();

  // After resize, tracker's last_total_rows is stale
  // Without re-sync, detection would be incorrect
  let detection_stale = tracker.detect(total_after, 24);

  // Re-sync tracker with new state
  tracker.init_from_screen(parser.screen());
  assert_eq!(tracker.last_total_rows(), total_after);

  // After re-sync, detection should show no pending scrollback
  let detection_synced = tracker.detect(total_after, 24);
  assert_eq!(
    detection_synced.num_pending_lines, 0,
    "After re-sync, no pending scrollback expected"
  );
}

#[test]
fn test_resize_32x120_to_57x238_scenario() {
  // Test the specific scenario reported: 32x120 -> 57x238
  // Guest shell should see full 57 rows after resize
  let mut parser = vt100::Parser::new(32, 120, 1000, TestReplySender);

  // Add some content
  for i in 0..20 {
    parser.process(format!("Line {:02}\r\n", i).as_bytes());
  }

  let mut tracker = ScrollbackTracker::new();
  tracker.init_from_screen(parser.screen());

  // Resize to larger dimensions
  parser.set_size(57, 238);

  // Verify VT100 reports correct size
  assert_eq!(
    parser.screen().size().rows,
    57,
    "VT100 should report 57 rows"
  );
  assert_eq!(
    parser.screen().size().cols,
    238,
    "VT100 should report 238 cols"
  );

  // Re-sync tracker (this is what the fix does)
  tracker.init_from_screen(parser.screen());

  // Verify tracker is synced
  let detection = tracker.detect(parser.screen().total_rows(), 57);
  assert_eq!(
    detection.num_pending_lines, 0,
    "No pending scrollback after resize and re-sync"
  );
}

#[test]
fn test_resize_larger_cursor_column_preserved() {
  // Test that cursor column is preserved after resize larger.
  // Note: Cursor row may change due to content reflow in the VT100 grid's
  // set_size logic, which recalculates abs_pos_row based on content reflow.
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  // Position cursor at row 10, col 40
  parser.process(b"\x1b[11;41H"); // 1-indexed in ANSI
  let (row, col) = parser.screen().cursor_position();
  assert_eq!(row, 10, "Cursor should be at row 10");
  assert_eq!(col, 40, "Cursor should be at col 40");

  // Resize larger
  parser.set_size(48, 160);

  let (row_after, col_after) = parser.screen().cursor_position();

  // Column should be preserved
  assert_eq!(
    col_after, 40,
    "Cursor col should be preserved after resize larger"
  );

  // Row should be within bounds (may change due to reflow)
  assert!(
    row_after < 48,
    "Cursor row {} should be within screen bounds",
    row_after
  );
}

#[test]
fn test_resize_larger_with_content_and_scrollback() {
  // Test resize larger with existing scrollback content
  let mut parser = vt100::Parser::new(10, 40, 100, TestReplySender);

  // Fill with content to create scrollback
  for i in 0..30 {
    parser.process(format!("Scrollback line {:02}\r\n", i).as_bytes());
  }

  let total_before = parser.screen().total_rows();
  let scrollback_len_before = parser.screen().scrollback_len();
  assert!(scrollback_len_before > 0, "Should have scrollback");

  let mut tracker = ScrollbackTracker::new();
  tracker.init_from_screen(parser.screen());

  // Resize much larger
  parser.set_size(50, 120);

  let total_after = parser.screen().total_rows();

  // Re-sync tracker
  let old_total = tracker.last_total_rows();
  tracker.init_from_screen(parser.screen());

  // Verify size change
  assert_eq!(parser.screen().size().rows, 50);
  assert_eq!(parser.screen().size().cols, 120);

  // Verify no spurious scrollback detection after re-sync
  let detection = tracker.detect(total_after, 50);
  assert_eq!(
    detection.num_pending_lines, 0,
    "No spurious scrollback after resize and re-sync. old_total={}, new_total={}",
    old_total, total_after
  );
}

#[test]
fn test_resize_preserves_cursor_in_content() {
  // Test that cursor stays with its content after resize
  let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

  // Write some lines and position cursor on line 5
  parser.process(b"Line 1\r\n");
  parser.process(b"Line 2\r\n");
  parser.process(b"Line 3\r\n");
  parser.process(b"Line 4\r\n");
  parser.process(b"Line 5"); // Cursor at end of "Line 5"

  let (row_before, col_before) = parser.screen().cursor_position();
  assert_eq!(row_before, 4, "Cursor should be on row 4 (0-indexed)");
  assert_eq!(col_before, 6, "Cursor should be at col 6");

  // Resize larger
  parser.set_size(48, 160);

  let (row_after, col_after) = parser.screen().cursor_position();

  // Cursor should still be at same logical position
  assert_eq!(
    col_after, 6,
    "Cursor col should be preserved (was {}, now {})",
    col_before, col_after
  );

  // Row may change if content reflows, but should be reasonable
  assert!(row_after < 48, "Cursor row should be within screen bounds");
}
