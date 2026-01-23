// Integration tests for scrollback injection functionality
//
// These tests verify that the scrollback module correctly:
// - Detects when content scrolls off the VT100 visible area
// - Extracts the scrolled lines from the VT100 buffer
// - Renders them to a tui buffer (simulating host terminal)
//
// Tests use the actual production code from src/scrollback.rs and render
// through ratatui's TestBackend to verify buffer contents.

use super::*;
use crate::scrollback::{
  ScrollbackTracker, process_scrollback, render_scrollback_to_buffer,
};
use crate::vt100::{self, TermReplySender};
use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::widgets::Widget;

/// Simple reply sender for testing (no-op implementation)
#[derive(Clone)]
struct TestReplySender;

impl TermReplySender for TestReplySender {
  fn reply(&self, _reply: compact_str::CompactString) {
    // No-op for testing
  }
}

/// Terminal widget for rendering VT100 screen to tui buffer (visible area only)
struct TerminalWidget<'a, T: TermReplySender> {
  screen: &'a vt100::Screen<T>,
}

impl<'a, T: TermReplySender> TerminalWidget<'a, T> {
  fn new(screen: &'a vt100::Screen<T>) -> Self {
    Self { screen }
  }
}

impl<T: TermReplySender> Widget for TerminalWidget<'_, T> {
  fn render(self, area: Rect, buf: &mut Buffer) {
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

/// Helper to extract text from a tui buffer
fn extract_buffer_text(buf: &Buffer) -> Vec<String> {
  let area = buf.area();
  let mut lines = Vec::new();
  for y in 0..area.height {
    let mut line = String::new();
    for x in 0..area.width {
      if let Some(cell) = buf.cell((area.x + x, area.y + y)) {
        line.push_str(cell.symbol());
      }
    }
    lines.push(line.trim_end().to_string());
  }
  lines
}

/// Helper to check if a buffer contains a specific string on any line
fn buffer_contains(buf: &Buffer, text: &str) -> bool {
  extract_buffer_text(buf)
    .iter()
    .any(|line| line.contains(text))
}

/// Helper to get a specific line from the buffer
fn buffer_line(buf: &Buffer, row: u16) -> String {
  let area = buf.area();
  let mut line = String::new();
  for x in 0..area.width {
    if let Some(cell) = buf.cell((area.x + x, area.y + row)) {
      line.push_str(cell.symbol());
    }
  }
  line.trim_end().to_string()
}

// =============================================================================
// Integration Tests Using Production Code
// =============================================================================

#[test]
fn test_process_scrollback_no_content() {
  // No content = no scrollback to process
  let parser = vt100::Parser::new(10, 80, 100, TestReplySender);
  let screen = parser.screen();

  let mut tracker = ScrollbackTracker::new();
  tracker.init_from_screen(screen);

  let mut buf = Buffer::empty(Rect::new(0, 0, 80, 10));
  let area = Rect::new(0, 0, 80, 10);

  let scroll_lines = process_scrollback(&mut tracker, screen, &mut buf, area);

  assert_eq!(scroll_lines, 0);
  assert!(!tracker.has_pending_scrollback());
}

#[test]
fn test_process_scrollback_content_within_screen() {
  // Content fits on screen - no scrollback
  let mut parser = vt100::Parser::new(10, 80, 100, TestReplySender);

  for i in 0..5 {
    parser.process(format!("Line {}\r\n", i).as_bytes());
  }

  let screen = parser.screen();
  let mut tracker = ScrollbackTracker::new();
  tracker.init_from_screen(screen);

  let mut buf = Buffer::empty(Rect::new(0, 0, 80, 10));
  let area = Rect::new(0, 0, 80, 10);

  let scroll_lines = process_scrollback(&mut tracker, screen, &mut buf, area);

  assert_eq!(scroll_lines, 0);
}

#[test]
fn test_process_scrollback_single_line_overflow() {
  // One line scrolls off - should be rendered to buffer
  let mut parser = vt100::Parser::new(5, 80, 100, TestReplySender);

  // Initialize tracker BEFORE adding content
  let mut tracker = ScrollbackTracker::new();
  tracker.init_from_screen(parser.screen());

  // Add 6 lines to a 5-row screen (1 scrolls off + cursor row)
  for i in 0..6 {
    parser.process(format!("Line {}\r\n", i).as_bytes());
  }

  let screen = parser.screen();
  let mut buf = Buffer::empty(Rect::new(0, 0, 80, 5));
  let area = Rect::new(0, 0, 80, 5);

  let scroll_lines = process_scrollback(&mut tracker, screen, &mut buf, area);

  // Should have scrolled lines
  assert!(scroll_lines > 0);

  // First line should be in buffer (scrollback rendered)
  assert!(
    buffer_contains(&buf, "Line 0"),
    "Buffer should contain 'Line 0' (scrollback line)"
  );
}

#[test]
fn test_process_scrollback_multiple_lines() {
  let mut parser = vt100::Parser::new(5, 80, 100, TestReplySender);

  let mut tracker = ScrollbackTracker::new();
  tracker.init_from_screen(parser.screen());

  // Add 10 lines to a 5-row screen
  for i in 0..10 {
    parser.process(format!("Line {:02}\r\n", i).as_bytes());
  }

  let screen = parser.screen();
  let mut buf = Buffer::empty(Rect::new(0, 0, 80, 5));
  let area = Rect::new(0, 0, 80, 5);

  let scroll_lines = process_scrollback(&mut tracker, screen, &mut buf, area);

  // Should have scrolled multiple lines
  assert!(scroll_lines >= 5);

  // Multiple scrollback lines should be in buffer
  let buf_text = extract_buffer_text(&buf);
  let has_scrollback = buf_text.iter().any(|l| l.contains("Line 0"));
  assert!(has_scrollback, "Buffer should contain scrollback lines");
}

#[test]
fn test_process_scrollback_updates_tracker() {
  let mut parser = vt100::Parser::new(5, 80, 100, TestReplySender);

  let mut tracker = ScrollbackTracker::new();
  tracker.init_from_screen(parser.screen());

  let initial_total = tracker.last_total_rows();

  // Add content
  for i in 0..8 {
    parser.process(format!("Line {}\r\n", i).as_bytes());
  }

  let screen = parser.screen();
  let mut buf = Buffer::empty(Rect::new(0, 0, 80, 5));
  let area = Rect::new(0, 0, 80, 5);

  process_scrollback(&mut tracker, screen, &mut buf, area);

  // Tracker should have updated
  assert!(
    tracker.last_total_rows() > initial_total,
    "Tracker should have advanced"
  );
}

#[test]
fn test_process_scrollback_pending_flag() {
  let mut parser = vt100::Parser::new(3, 80, 100, TestReplySender);

  let mut tracker = ScrollbackTracker::new();
  tracker.init_from_screen(parser.screen());

  // Add many lines (more than can be processed in one frame)
  for i in 0..20 {
    parser.process(format!("Line {:02}\r\n", i).as_bytes());
  }

  let screen = parser.screen();
  let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
  let area = Rect::new(0, 0, 80, 3);

  let scroll_lines = process_scrollback(&mut tracker, screen, &mut buf, area);

  // More lines pending than screen height
  assert!(scroll_lines > 0);
  assert!(
    tracker.has_pending_scrollback(),
    "Should have pending scrollback"
  );
}

#[test]
fn test_process_scrollback_multiple_frames() {
  let mut parser = vt100::Parser::new(5, 80, 100, TestReplySender);

  let mut tracker = ScrollbackTracker::new();
  tracker.init_from_screen(parser.screen());

  // Add many lines
  for i in 0..25 {
    parser.process(format!("Line {:02}\r\n", i).as_bytes());
  }

  let screen = parser.screen();
  let area = Rect::new(0, 0, 80, 5);

  // Process multiple frames until no pending
  let mut frame_count = 0;
  while tracker.has_pending_scrollback() || frame_count == 0 {
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 5));
    let scroll_lines = process_scrollback(&mut tracker, screen, &mut buf, area);

    if frame_count == 0 {
      assert!(scroll_lines > 0, "First frame should scroll");
    }

    frame_count += 1;
    if frame_count > 10 {
      break; // Safety limit
    }
  }

  assert!(!tracker.has_pending_scrollback(), "Should have no pending");
  assert_eq!(
    tracker.last_total_rows(),
    screen.total_rows(),
    "Tracker should be in sync"
  );
}

// =============================================================================
// render_scrollback_to_buffer Tests
// =============================================================================

#[test]
fn test_render_scrollback_basic() {
  let mut parser = vt100::Parser::new(5, 40, 100, TestReplySender);

  // Create scrollback
  for i in 0..10 {
    parser.process(format!("Scrollback Line {}\r\n", i).as_bytes());
  }

  let screen = parser.screen();
  let row0 = screen.row0();

  // Calculate range for first 3 scrollback lines
  let range = ScrollbackTracker::calculate_range(row0, row0, 3);

  let mut buf = Buffer::empty(Rect::new(0, 0, 40, 5));
  let lines_rendered =
    render_scrollback_to_buffer(screen, &range, &mut buf, 0, 0, 40);

  assert_eq!(lines_rendered, 3);

  // Verify content
  assert!(
    buffer_contains(&buf, "Scrollback Line 0"),
    "Should contain first scrollback line"
  );
}

#[test]
fn test_render_scrollback_with_offset() {
  let mut parser = vt100::Parser::new(5, 40, 100, TestReplySender);

  for i in 0..10 {
    parser.process(format!("Line {}\r\n", i).as_bytes());
  }

  let screen = parser.screen();
  let row0 = screen.row0();
  let range = ScrollbackTracker::calculate_range(row0, row0, 2);

  // Render at offset (5, 2)
  let mut buf = Buffer::empty(Rect::new(0, 0, 50, 10));
  let lines_rendered =
    render_scrollback_to_buffer(screen, &range, &mut buf, 5, 2, 40);

  assert_eq!(lines_rendered, 2);

  // Content should be at row 2, starting at column 5
  let line2 = buffer_line(&buf, 2);
  assert!(
    line2.contains("Line 0"),
    "Line 0 should be at row 2: '{}'",
    line2
  );
}

#[test]
fn test_render_scrollback_width_limit() {
  let mut parser = vt100::Parser::new(5, 80, 100, TestReplySender);

  // Create long line
  parser.process(b"A".repeat(80).as_slice());
  parser.process(b"\r\n");
  for _ in 0..6 {
    parser.process(b"X\r\n");
  }

  let screen = parser.screen();
  let row0 = screen.row0();
  let range = ScrollbackTracker::calculate_range(row0, row0, 1);

  // Render with width limit of 20
  let mut buf = Buffer::empty(Rect::new(0, 0, 20, 5));
  render_scrollback_to_buffer(screen, &range, &mut buf, 0, 0, 20);

  // Line should be truncated to 20 chars
  let line0 = buffer_line(&buf, 0);
  assert!(
    line0.len() <= 20,
    "Line should be max 20 chars, got {}",
    line0.len()
  );
}

// =============================================================================
// Integration with TestHarness
// =============================================================================

#[test]
fn test_scrollback_with_harness() {
  let mut harness =
    TestHarness::with_config(TestAppConfig::new().with_terminal_size(80, 5));
  let mut parser = vt100::Parser::new(5, 80, 100, TestReplySender);

  let mut tracker = ScrollbackTracker::new();
  tracker.init_from_screen(parser.screen());

  // Add content that scrolls
  for i in 0..8 {
    parser.process(format!("Test Line {}\r\n", i).as_bytes());
  }

  let screen = parser.screen();

  // Render scrollback to harness buffer
  harness
    .terminal
    .draw(|f| {
      let area = f.area();
      let buf = f.buffer_mut();
      process_scrollback(&mut tracker, screen, buf, area);
    })
    .expect("Should render");

  // Verify scrollback was rendered
  harness.assert_buffer_contains("Test Line 0");
}

#[test]
fn test_scrollback_then_visible_rendering() {
  let mut harness =
    TestHarness::with_config(TestAppConfig::new().with_terminal_size(80, 5));
  let mut parser = vt100::Parser::new(5, 80, 100, TestReplySender);

  let mut tracker = ScrollbackTracker::new();
  tracker.init_from_screen(parser.screen());

  // Add scrolling content
  for i in 0..10 {
    parser.process(format!("Line {:02}\r\n", i).as_bytes());
  }

  let screen = parser.screen();

  // First: render scrollback
  harness
    .terminal
    .draw(|f| {
      let area = f.area();
      let buf = f.buffer_mut();
      process_scrollback(&mut tracker, screen, buf, area);
    })
    .expect("Scrollback render");

  // Second: render visible screen
  harness
    .terminal
    .draw(|f| {
      let area = f.area();
      let widget = TerminalWidget::new(screen);
      f.render_widget(widget, area);
    })
    .expect("Visible render");

  // Visible content should now be in buffer
  // (scrollback was overwritten by visible render, which is correct behavior)
  let buffer_str = harness.buffer_as_string();
  // Most recent lines should be visible
  assert!(
    buffer_str.contains("Line 09") || buffer_str.contains("Line 08"),
    "Buffer should contain recent lines: {}",
    buffer_str
  );
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn test_scrollback_empty_lines() {
  let mut parser = vt100::Parser::new(5, 40, 100, TestReplySender);

  let mut tracker = ScrollbackTracker::new();
  tracker.init_from_screen(parser.screen());

  // Mix of content and empty lines
  parser.process(b"Content\r\n");
  parser.process(b"\r\n");
  parser.process(b"More Content\r\n");
  parser.process(b"\r\n");
  parser.process(b"\r\n");
  parser.process(b"End\r\n");
  parser.process(b"Extra\r\n");
  parser.process(b"Lines\r\n");

  let screen = parser.screen();
  let mut buf = Buffer::empty(Rect::new(0, 0, 40, 5));
  let area = Rect::new(0, 0, 40, 5);

  let _scroll_lines = process_scrollback(&mut tracker, screen, &mut buf, area);

  // Should process without crashing - if we got here, it worked
  // (scroll_lines is u16 so always >= 0)
}

#[test]
fn test_scrollback_ansi_codes() {
  let mut parser = vt100::Parser::new(5, 40, 100, TestReplySender);

  let mut tracker = ScrollbackTracker::new();
  tracker.init_from_screen(parser.screen());

  // Content with ANSI codes
  parser.process(b"\x1b[31mRed Text\x1b[0m\r\n");
  parser.process(b"\x1b[32mGreen Text\x1b[0m\r\n");
  parser.process(b"\x1b[1mBold\x1b[0m\r\n");
  parser.process(b"Normal 1\r\n");
  parser.process(b"Normal 2\r\n");
  parser.process(b"Normal 3\r\n");
  parser.process(b"Normal 4\r\n");
  parser.process(b"Normal 5\r\n");

  let screen = parser.screen();
  let mut buf = Buffer::empty(Rect::new(0, 0, 40, 5));
  let area = Rect::new(0, 0, 40, 5);

  process_scrollback(&mut tracker, screen, &mut buf, area);

  // Text content should be rendered (without raw ANSI codes)
  let buf_text = extract_buffer_text(&buf).join("\n");
  assert!(
    buf_text.contains("Red Text") || buf_text.contains("Green Text"),
    "Should contain text from ANSI lines"
  );
}

#[test]
fn test_scrollback_rapid_content() {
  let mut parser = vt100::Parser::new(5, 40, 100, TestReplySender);

  let mut tracker = ScrollbackTracker::new();
  tracker.init_from_screen(parser.screen());

  // Bulk content
  let mut content = String::new();
  for i in 0..50 {
    content.push_str(&format!("Bulk Line {:02}\r\n", i));
  }
  parser.process(content.as_bytes());

  let screen = parser.screen();
  let area = Rect::new(0, 0, 40, 5);

  // Process all in one go
  let mut total_scrolled = 0;
  for _ in 0..20 {
    // Safety limit
    let mut buf = Buffer::empty(Rect::new(0, 0, 40, 5));
    let scrolled = process_scrollback(&mut tracker, screen, &mut buf, area);
    total_scrolled += scrolled;

    if !tracker.has_pending_scrollback() {
      break;
    }
  }

  assert!(total_scrolled > 0, "Should have scrolled content");
  assert_eq!(tracker.last_total_rows(), screen.total_rows());
}

#[test]
fn test_scrollback_incremental_content() {
  let mut parser = vt100::Parser::new(5, 40, 100, TestReplySender);

  let mut tracker = ScrollbackTracker::new();
  tracker.init_from_screen(parser.screen());

  let area = Rect::new(0, 0, 40, 5);

  // Add content incrementally, processing after each batch
  for batch in 0..5 {
    for line in 0..3 {
      parser.process(format!("B{} L{}\r\n", batch, line).as_bytes());
    }

    let screen = parser.screen();
    let mut buf = Buffer::empty(Rect::new(0, 0, 40, 5));
    process_scrollback(&mut tracker, screen, &mut buf, area);

    // Tracker should stay in sync
    assert!(
      tracker.last_total_rows() <= screen.total_rows(),
      "Tracker should not exceed screen total"
    );
  }

  // Final state should be in sync
  assert_eq!(tracker.last_total_rows(), parser.screen().total_rows());
}

// =============================================================================
// Scrollback Content Verification
// =============================================================================

#[test]
fn test_scrollback_content_order() {
  let mut parser = vt100::Parser::new(5, 40, 100, TestReplySender);

  let mut tracker = ScrollbackTracker::new();
  tracker.init_from_screen(parser.screen());

  // Add numbered lines
  for i in 0..10 {
    parser.process(format!("Numbered {:02}\r\n", i).as_bytes());
  }

  let screen = parser.screen();
  let row0 = screen.row0();

  // Get all scrollback
  let range = ScrollbackTracker::calculate_range(row0, row0, row0);

  let mut buf = Buffer::empty(Rect::new(0, 0, 40, row0 as u16));
  render_scrollback_to_buffer(screen, &range, &mut buf, 0, 0, 40);

  let lines = extract_buffer_text(&buf);

  // Verify lines are in order (oldest first)
  let mut prev_num = -1i32;
  for line in &lines {
    if line.starts_with("Numbered ") {
      if let Some(num_str) = line.strip_prefix("Numbered ") {
        if let Ok(num) = num_str.trim().parse::<i32>() {
          assert!(
            num > prev_num,
            "Lines should be in order: {} should be > {}",
            num,
            prev_num
          );
          prev_num = num;
        }
      }
    }
  }
}

#[test]
fn test_scrollback_preserves_text_exactly() {
  let mut parser = vt100::Parser::new(5, 80, 100, TestReplySender);

  let mut tracker = ScrollbackTracker::new();
  tracker.init_from_screen(parser.screen());

  // Add specific text
  let test_strings = [
    "Hello, World!",
    "Special chars: @#$%^&*()",
    "Numbers: 1234567890",
    "Mixed: abc123XYZ",
    "Spaces   in   text",
    "More content here",
    "Even more lines",
    "Final test line",
  ];

  for s in &test_strings {
    parser.process(format!("{}\r\n", s).as_bytes());
  }

  let screen = parser.screen();
  let row0 = screen.row0();
  let range = ScrollbackTracker::calculate_range(row0, row0, row0);

  let mut buf = Buffer::empty(Rect::new(0, 0, 80, row0 as u16));
  render_scrollback_to_buffer(screen, &range, &mut buf, 0, 0, 80);

  // Verify content is preserved
  for s in test_strings.iter().take(row0) {
    assert!(buffer_contains(&buf, s), "Buffer should contain '{}'", s);
  }
}
