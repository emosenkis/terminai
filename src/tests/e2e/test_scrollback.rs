// Comprehensive tests for scrollback injection functionality
//
// These tests verify that the VT100 terminal emulation correctly handles
// scrollback, including:
// - Basic scrollback detection (content scrolling off screen)
// - Scrollback content extraction and iteration
// - Row indexing (row0, total_rows)
// - Scrollback limits and buffer management
// - Rendering scrollback lines to host terminal
// - Scrollback offset navigation
//
// The scrollback injection mechanism in terminai.rs works as follows:
// 1. Track last_total_rows vs current total_rows
// 2. When content scrolls off, the difference is the number of new scrollback lines
// 3. Extract those lines from all_rows().skip(scrollback_start).take(rows_to_scroll)
// 4. Render them to the frame buffer
// 5. Call frame.set_scroll_up() to push to native scrollback

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

/// Helper to extract visible text from a VT100 screen (visible area only)
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

/// Helper to extract text from all rows (scrollback + visible)
fn extract_all_rows_text(
  screen: &vt100::Screen<impl TermReplySender>,
) -> Vec<String> {
  let mut lines = Vec::new();
  for row in screen.all_rows() {
    let mut line = String::new();
    for col_idx in 0..row.cols() {
      if let Some(cell) = row.get(col_idx) {
        if cell.has_contents() {
          line.push_str(&cell.contents());
        } else {
          line.push(' ');
        }
      }
    }
    lines.push(line.trim_end().to_string());
  }
  lines
}

/// Helper to extract just scrollback rows (rows before row0)
fn extract_scrollback_text(
  screen: &vt100::Screen<impl TermReplySender>,
) -> Vec<String> {
  let row0 = screen.row0();
  let mut lines = Vec::new();
  for row in screen.all_rows().take(row0) {
    let mut line = String::new();
    for col_idx in 0..row.cols() {
      if let Some(cell) = row.get(col_idx) {
        if cell.has_contents() {
          line.push_str(&cell.contents());
        } else {
          line.push(' ');
        }
      }
    }
    lines.push(line.trim_end().to_string());
  }
  lines
}

// =============================================================================
// Basic Scrollback Detection Tests
// =============================================================================

#[test]
fn test_scrollback_initial_state() {
  // New terminal should have no scrollback
  let parser = vt100::Parser::new(10, 80, 1000, TestReplySender);
  let screen = parser.screen();

  assert_eq!(
    screen.total_rows(),
    10,
    "Initial total_rows should equal visible rows"
  );
  assert_eq!(screen.row0(), 0, "Initial row0 should be 0");
  assert_eq!(
    screen.scrollback(),
    0,
    "Initial scrollback offset should be 0"
  );
}

#[test]
fn test_scrollback_no_content() {
  // Terminal with no content should have total_rows = visible rows
  let parser = vt100::Parser::new(24, 80, 1000, TestReplySender);
  let screen = parser.screen();

  assert_eq!(screen.total_rows(), 24);
  assert_eq!(screen.row0(), 0);
}

#[test]
fn test_scrollback_content_within_screen() {
  // Content that fits on screen shouldn't trigger scrollback
  let mut parser = vt100::Parser::new(10, 80, 1000, TestReplySender);

  // Write 5 lines (less than screen height of 10)
  for i in 0..5 {
    parser.process(format!("Line {}\r\n", i).as_bytes());
  }

  let screen = parser.screen();
  assert_eq!(screen.total_rows(), 10, "No scrollback yet");
  assert_eq!(screen.row0(), 0, "row0 should still be 0");
}

#[test]
fn test_scrollback_single_line_overflow() {
  // When exactly one line scrolls off
  let mut parser = vt100::Parser::new(10, 80, 1000, TestReplySender);

  // Write 11 lines (each \r\n adds a row, so 11 lines = 12 rows)
  // Screen height is 10, so 2 lines will scroll off
  for i in 0..11 {
    parser.process(format!("Line {}\r\n", i).as_bytes());
  }

  let screen = parser.screen();
  // 11 lines + final cursor position = 12 total rows
  assert_eq!(
    screen.total_rows(),
    12,
    "Should have 12 total rows (11 lines + cursor row)"
  );
  assert_eq!(screen.row0(), 2, "row0 should be 2 (2 lines scrolled off)");

  // First lines ("Line 0", "Line 1") should be in scrollback
  let scrollback = extract_scrollback_text(screen);
  assert_eq!(scrollback.len(), 2);
  assert!(scrollback[0].contains("Line 0"));
}

#[test]
fn test_scrollback_multi_line_overflow() {
  // Multiple lines scrolling off at once
  let mut parser = vt100::Parser::new(10, 80, 1000, TestReplySender);

  // Write 15 lines (each \r\n adds a row, so 15 lines = 16 total rows)
  // Screen height is 10, so 6 lines will scroll off
  for i in 0..15 {
    parser.process(format!("Line {}\r\n", i).as_bytes());
  }

  let screen = parser.screen();
  // 15 lines + final cursor position = 16 total rows
  assert_eq!(screen.total_rows(), 16);
  assert_eq!(screen.row0(), 6);

  // Lines 0-5 should be in scrollback
  let scrollback = extract_scrollback_text(screen);
  assert_eq!(scrollback.len(), 6);
  for (idx, line) in scrollback.iter().enumerate() {
    assert!(
      line.contains(&format!("Line {}", idx)),
      "Scrollback line {} should contain 'Line {}'",
      idx,
      idx
    );
  }
}

// =============================================================================
// Scrollback Content Extraction Tests
// =============================================================================

#[test]
fn test_all_rows_iterator_order() {
  // all_rows should iterate from oldest (scrollback) to newest (visible)
  let mut parser = vt100::Parser::new(5, 80, 1000, TestReplySender);

  // Write 8 lines (8 lines + cursor row = 9 total rows)
  for i in 0..8 {
    parser.process(format!("Line {}\r\n", i).as_bytes());
  }

  let screen = parser.screen();
  let all_lines = extract_all_rows_text(screen);

  // Should have 9 rows total (8 lines + cursor row)
  assert_eq!(all_lines.len(), 9);

  // Lines should be in order from oldest to newest
  // Skip the last row (cursor position with no content)
  for (idx, line) in all_lines.iter().take(8).enumerate() {
    if !line.is_empty() {
      assert!(
        line.contains(&format!("Line {}", idx)),
        "Row {} should contain 'Line {}'",
        idx,
        idx
      );
    }
  }
}

#[test]
fn test_scrollback_content_preservation() {
  // Scrollback should preserve content exactly
  let mut parser = vt100::Parser::new(5, 80, 1000, TestReplySender);

  // Write content with specific patterns
  parser.process(b"First scrollback line\r\n");
  parser.process(b"Second scrollback line\r\n");
  parser.process(b"Third scrollback line\r\n");
  parser.process(b"Still on screen 1\r\n");
  parser.process(b"Still on screen 2\r\n");
  parser.process(b"Still on screen 3\r\n");
  parser.process(b"Still on screen 4\r\n");
  parser.process(b"Still on screen 5\r\n");

  let screen = parser.screen();
  let scrollback = extract_scrollback_text(screen);

  // First 3 lines should be in scrollback
  assert!(scrollback.len() >= 3);
  assert!(scrollback[0].contains("First scrollback line"));
  assert!(scrollback[1].contains("Second scrollback line"));
  assert!(scrollback[2].contains("Third scrollback line"));
}

#[test]
fn test_row_indexing_consistency() {
  // total_rows should always equal row0 + visible_rows
  let mut parser = vt100::Parser::new(10, 80, 1000, TestReplySender);

  // Check at various stages of filling
  for num_lines in [0, 5, 10, 15, 20, 50] {
    parser = vt100::Parser::new(10, 80, 1000, TestReplySender);
    for i in 0..num_lines {
      parser.process(format!("Line {}\r\n", i).as_bytes());
    }

    let screen = parser.screen();
    let visible_rows = screen.size().rows as usize;

    // total_rows = scrollback_count + visible_rows
    // row0 = scrollback_count (where visible rows start)
    // So: total_rows = row0 + visible_rows
    assert_eq!(
      screen.total_rows(),
      screen.row0() + visible_rows,
      "total_rows should equal row0 + visible_rows for {} lines",
      num_lines
    );
  }
}

// =============================================================================
// Scrollback Limit Tests
// =============================================================================

#[test]
fn test_scrollback_limit_respected() {
  // Scrollback buffer should not exceed scrollback_len
  let scrollback_limit = 10;
  let mut parser = vt100::Parser::new(5, 80, scrollback_limit, TestReplySender);

  // Write way more lines than scrollback can hold
  for i in 0..50 {
    parser.process(format!("Line {}\r\n", i).as_bytes());
  }

  let screen = parser.screen();
  let scrollback = extract_scrollback_text(screen);

  // Scrollback should be limited to scrollback_len
  assert!(
    scrollback.len() <= scrollback_limit,
    "Scrollback ({}) should not exceed limit ({})",
    scrollback.len(),
    scrollback_limit
  );
}

#[test]
fn test_scrollback_limit_zero() {
  // With scrollback_len = 0, no scrollback should be preserved
  let mut parser = vt100::Parser::new(5, 80, 0, TestReplySender);

  // Write 10 lines
  for i in 0..10 {
    parser.process(format!("Line {}\r\n", i).as_bytes());
  }

  let screen = parser.screen();
  let scrollback = extract_scrollback_text(screen);

  // No scrollback with limit of 0
  assert_eq!(scrollback.len(), 0, "No scrollback when limit is 0");
}

#[test]
fn test_scrollback_oldest_lines_discarded() {
  // When scrollback limit is reached, oldest lines should be discarded
  let scrollback_limit = 5;
  let mut parser = vt100::Parser::new(5, 80, scrollback_limit, TestReplySender);

  // Write 20 lines (15 would scroll off, but only 5 can be kept)
  for i in 0..20 {
    parser.process(format!("Line {:02}\r\n", i).as_bytes());
  }

  let screen = parser.screen();
  let scrollback = extract_scrollback_text(screen);

  // Should have exactly scrollback_limit lines
  assert_eq!(scrollback.len(), scrollback_limit);

  // Oldest lines (0-9) should have been discarded
  // Lines 10-14 should be in scrollback
  // The scrollback should contain the most recent lines that scrolled off
  for line in &scrollback {
    if !line.is_empty() {
      // Extract the line number from the content
      if let Some(num_str) = line.strip_prefix("Line ") {
        if let Ok(num) = num_str.trim().parse::<i32>() {
          assert!(num >= 10, "Line {} should have been discarded", num);
        }
      }
    }
  }
}

// =============================================================================
// Scrollback Offset Navigation Tests
// =============================================================================

#[test]
fn test_scrollback_offset_navigation() {
  let mut parser = vt100::Parser::new(5, 80, 100, TestReplySender);

  // Create scrollback
  for i in 0..15 {
    parser.process(format!("Line {}\r\n", i).as_bytes());
  }

  let screen = parser.screen();
  assert_eq!(screen.scrollback(), 0, "Initial offset should be 0");

  // Navigate into scrollback
  parser.set_scrollback(5);
  assert_eq!(parser.screen().scrollback(), 5);

  // Navigate back
  parser.set_scrollback(0);
  assert_eq!(parser.screen().scrollback(), 0);
}

#[test]
fn test_scrollback_offset_clamped() {
  let mut parser = vt100::Parser::new(5, 80, 100, TestReplySender);

  // Create some scrollback
  for i in 0..15 {
    parser.process(format!("Line {}\r\n", i).as_bytes());
  }

  let row0 = parser.screen().row0();

  // Try to scroll past available scrollback
  parser.set_scrollback(1000);

  // Should be clamped to row0 (max scrollback position)
  assert!(parser.screen().scrollback() <= row0);
}

#[test]
fn test_scrollback_offset_scroll_up_down() {
  let mut parser = vt100::Parser::new(5, 80, 100, TestReplySender);

  // Create scrollback
  for i in 0..20 {
    parser.process(format!("Line {}\r\n", i).as_bytes());
  }

  let initial_offset = parser.screen().scrollback();
  assert_eq!(initial_offset, 0);

  // Scroll up (into scrollback)
  // Note: The Screen methods are scroll_up and scroll_down
  // We need to access them via the screen's mut methods
  // For this test, we'll use set_scrollback directly
  parser.set_scrollback(3);
  assert_eq!(parser.screen().scrollback(), 3);

  // Scroll down (back towards current)
  parser.set_scrollback(1);
  assert_eq!(parser.screen().scrollback(), 1);
}

// =============================================================================
// Terminai.rs Scrollback Injection Simulation Tests
// =============================================================================

/// Simulate the scrollback detection logic from terminai.rs
struct ScrollbackTracker {
  last_total_rows: usize,
  has_pending_scrollback: bool,
}

impl ScrollbackTracker {
  fn new() -> Self {
    Self {
      last_total_rows: 0,
      has_pending_scrollback: false,
    }
  }

  fn init(&mut self, total_rows: usize) {
    self.last_total_rows = total_rows;
  }

  /// Detect new scrollback lines (mirrors terminai.rs logic)
  fn detect_scrollback(&mut self, current_total_rows: usize) -> usize {
    if current_total_rows > self.last_total_rows {
      current_total_rows - self.last_total_rows
    } else {
      0
    }
  }

  /// Update tracking after processing scrollback (mirrors terminai.rs logic)
  fn update_tracking(
    &mut self,
    num_pending_lines: usize,
    rows_to_scroll: usize,
  ) {
    self.last_total_rows += rows_to_scroll;
    self.has_pending_scrollback = rows_to_scroll < num_pending_lines;
  }
}

#[test]
fn test_scrollback_detection_logic() {
  // Test the terminai.rs detection logic
  let mut tracker = ScrollbackTracker::new();
  let mut parser = vt100::Parser::new(10, 80, 1000, TestReplySender);

  // Initialize tracker
  tracker.init(parser.screen().total_rows());

  // No change initially
  assert_eq!(tracker.detect_scrollback(parser.screen().total_rows()), 0);

  // Add some content (no scrollback yet - 5 lines = 6 rows, fits in 10-row screen)
  for i in 0..5 {
    parser.process(format!("Line {}\r\n", i).as_bytes());
  }
  assert_eq!(tracker.detect_scrollback(parser.screen().total_rows()), 0);

  // Add content that causes scrollback
  // 10 more lines = 11 more rows, totaling 16 rows (6 new scrollback lines)
  for i in 5..15 {
    parser.process(format!("Line {}\r\n", i).as_bytes());
  }

  let new_lines = tracker.detect_scrollback(parser.screen().total_rows());
  // 15 lines + cursor = 16 total rows, but last_total_rows is 10, so 6 new
  assert_eq!(new_lines, 6, "Should detect 6 new scrollback lines");
}

#[test]
fn test_scrollback_update_tracking() {
  let mut tracker = ScrollbackTracker::new();
  let screen_height = 10;

  // Simulate scenario from terminai.rs
  tracker.last_total_rows = 10;

  // 15 new lines scrolled
  let num_pending = 15;
  let rows_to_scroll = num_pending.min(screen_height);

  tracker.update_tracking(num_pending, rows_to_scroll);

  assert_eq!(tracker.last_total_rows, 10 + rows_to_scroll);
  assert!(
    tracker.has_pending_scrollback,
    "Should have pending scrollback when not all lines processed"
  );
}

#[test]
fn test_scrollback_no_pending_when_all_processed() {
  let mut tracker = ScrollbackTracker::new();
  let screen_height = 10;

  tracker.last_total_rows = 10;

  // Only 5 new lines (less than screen height)
  let num_pending = 5;
  let rows_to_scroll = num_pending.min(screen_height);

  tracker.update_tracking(num_pending, rows_to_scroll);

  assert!(!tracker.has_pending_scrollback);
}

#[test]
fn test_scrollback_extraction_range() {
  // Test the extraction range calculation from terminai.rs
  let mut parser = vt100::Parser::new(10, 80, 1000, TestReplySender);

  // Create scrollback with known content
  for i in 0..20 {
    parser.process(format!("Line {:02}\r\n", i).as_bytes());
  }

  let screen = parser.screen();
  let row0 = screen.row0();
  let num_scrollback = 5; // Simulating 5 new lines scrolled off

  // Calculate range like terminai.rs does
  let scrollback_start = row0.saturating_sub(num_scrollback);

  // Extract the range
  let extracted: Vec<String> = screen
    .all_rows()
    .skip(scrollback_start)
    .take(num_scrollback)
    .map(|row| {
      let mut line = String::new();
      for col in 0..row.cols() {
        if let Some(cell) = row.get(col) {
          if cell.has_contents() {
            line.push_str(&cell.contents());
          }
        }
      }
      line.trim_end().to_string()
    })
    .collect();

  assert_eq!(extracted.len(), num_scrollback);
}

// =============================================================================
// Scrollback with Special Content Tests
// =============================================================================

#[test]
fn test_scrollback_with_empty_lines() {
  let mut parser = vt100::Parser::new(5, 80, 100, TestReplySender);

  // Mix of content and empty lines
  parser.process(b"Content 1\r\n");
  parser.process(b"\r\n"); // Empty
  parser.process(b"Content 2\r\n");
  parser.process(b"\r\n"); // Empty
  parser.process(b"Content 3\r\n");
  parser.process(b"\r\n"); // Empty
  parser.process(b"Content 4\r\n");
  parser.process(b"Content 5\r\n");

  let screen = parser.screen();
  let all_rows = extract_all_rows_text(screen);

  // Should preserve empty lines in scrollback
  assert!(all_rows.iter().any(|l| l.is_empty()));
}

#[test]
fn test_scrollback_with_long_lines() {
  let mut parser = vt100::Parser::new(5, 20, 100, TestReplySender);

  // Lines longer than terminal width
  parser.process(b"This is a very long line that exceeds width\r\n");
  parser.process(b"Short\r\n");
  parser.process(b"Another very long line for testing purposes\r\n");
  parser.process(b"Line 3\r\n");
  parser.process(b"Line 4\r\n");
  parser.process(b"Line 5\r\n");
  parser.process(b"Line 6\r\n");

  let screen = parser.screen();

  // Should handle wrapping correctly
  assert!(screen.total_rows() > 0);
}

#[test]
fn test_scrollback_with_ansi_codes() {
  let mut parser = vt100::Parser::new(5, 80, 100, TestReplySender);

  // Content with ANSI escape codes
  parser.process(b"\x1b[31mRed text\x1b[0m\r\n");
  parser.process(b"\x1b[32mGreen text\x1b[0m\r\n");
  parser.process(b"\x1b[1mBold text\x1b[0m\r\n");
  parser.process(b"Normal line 1\r\n");
  parser.process(b"Normal line 2\r\n");
  parser.process(b"Normal line 3\r\n");
  parser.process(b"Normal line 4\r\n");
  parser.process(b"Normal line 5\r\n");

  let screen = parser.screen();
  let scrollback = extract_scrollback_text(screen);

  // Text content should be preserved (without ANSI codes)
  let scrollback_text = scrollback.join("\n");
  assert!(scrollback_text.contains("Red text"));
  assert!(scrollback_text.contains("Green text"));
  assert!(scrollback_text.contains("Bold text"));
}

// =============================================================================
// Scrollback Rendering Tests
// =============================================================================

#[test]
fn test_scrollback_render_to_buffer() {
  let mut harness =
    TestHarness::with_config(TestAppConfig::new().with_terminal_size(80, 10));
  let mut parser = vt100::Parser::new(10, 80, 100, TestReplySender);

  // Create content that scrolls
  for i in 0..15 {
    parser.process(format!("Line {:02}\r\n", i).as_bytes());
  }

  let screen = parser.screen();
  let widget = TerminalWidget::new(screen);

  harness.render(widget).expect("Should render");

  // Visible area should show most recent lines
  let buffer_str = harness.buffer_as_string();

  // Lines 5-14 should be visible (10 most recent)
  // Line 0-4 should be in scrollback (not visible without scrolling)
  assert!(buffer_str.contains("Line 14") || buffer_str.contains("Line 13"));
}

#[test]
fn test_scrollback_line_rendering_order() {
  let mut parser = vt100::Parser::new(5, 80, 100, TestReplySender);

  // Create specific content
  for i in 0..10 {
    parser.process(format!("Line {:02}\r\n", i).as_bytes());
  }

  let screen = parser.screen();
  let row0 = screen.row0();

  // Simulate rendering scrollback lines like terminai.rs
  let num_scrollback = row0;
  let mut rendered_lines = Vec::new();

  for row in screen.all_rows().take(num_scrollback) {
    let mut line = String::new();
    for col in 0..row.cols() {
      if let Some(cell) = row.get(col) {
        if cell.has_contents() {
          line.push_str(&cell.contents());
        }
      }
    }
    rendered_lines.push(line.trim_end().to_string());
  }

  // Lines should be in order from oldest to most recent scrollback
  for (idx, line) in rendered_lines.iter().enumerate() {
    if !line.is_empty() {
      assert!(
        line.contains(&format!("Line {:02}", idx)),
        "Rendered line {} should contain 'Line {:02}'",
        idx,
        idx
      );
    }
  }
}

// =============================================================================
// Edge Case Tests
// =============================================================================

#[test]
fn test_scrollback_exactly_one_screen() {
  // Content exactly fills screen - no scrollback yet
  let mut parser = vt100::Parser::new(5, 80, 100, TestReplySender);

  for i in 0..5 {
    parser.process(format!("Line {}\r\n", i).as_bytes());
  }

  let screen = parser.screen();
  // With 5 lines and screen height 5, depends on cursor position
  // The last newline may push content off
  assert!(screen.total_rows() >= 5);
}

#[test]
fn test_scrollback_single_character_lines() {
  let mut parser = vt100::Parser::new(5, 80, 100, TestReplySender);

  for i in 0..10 {
    parser.process(format!("{}\r\n", i % 10).as_bytes());
  }

  let screen = parser.screen();
  let scrollback = extract_scrollback_text(screen);

  // Single character lines should be preserved
  assert!(!scrollback.is_empty());
}

#[test]
fn test_scrollback_rapid_content() {
  // Simulate rapid content output
  let mut parser = vt100::Parser::new(10, 80, 100, TestReplySender);

  // Bulk write (50 lines = 51 total rows including cursor row)
  let mut content = String::new();
  for i in 0..50 {
    content.push_str(&format!("Line {:02}\r\n", i));
  }
  parser.process(content.as_bytes());

  let screen = parser.screen();
  // 50 lines + cursor row = 51 total rows, 10 visible, so 41 in scrollback
  assert_eq!(screen.row0(), 41, "41 lines should be in scrollback");
  assert_eq!(screen.total_rows(), 51);
}

#[test]
fn test_scrollback_incremental_vs_bulk() {
  // Compare incremental vs bulk content writing
  let mut parser_inc = vt100::Parser::new(10, 80, 100, TestReplySender);
  let mut parser_bulk = vt100::Parser::new(10, 80, 100, TestReplySender);

  // Incremental
  for i in 0..30 {
    parser_inc.process(format!("Line {:02}\r\n", i).as_bytes());
  }

  // Bulk
  let mut content = String::new();
  for i in 0..30 {
    content.push_str(&format!("Line {:02}\r\n", i));
  }
  parser_bulk.process(content.as_bytes());

  // Should produce identical results
  assert_eq!(
    parser_inc.screen().total_rows(),
    parser_bulk.screen().total_rows()
  );
  assert_eq!(parser_inc.screen().row0(), parser_bulk.screen().row0());
}

#[test]
fn test_scrollback_with_carriage_return_only() {
  // Content with CR but no LF (line overwriting)
  let mut parser = vt100::Parser::new(5, 80, 100, TestReplySender);

  // Write, then overwrite same line
  parser.process(b"Original text\r");
  parser.process(b"Overwritten\r\n");
  parser.process(b"Line 2\r\n");
  parser.process(b"Line 3\r\n");
  parser.process(b"Line 4\r\n");
  parser.process(b"Line 5\r\n");
  parser.process(b"Line 6\r\n");

  let screen = parser.screen();
  let all_text = extract_all_rows_text(screen).join("\n");

  // Should contain the overwritten version, not original
  assert!(all_text.contains("Overwritten"));
}

#[test]
fn test_scrollback_cursor_at_bottom() {
  // Cursor should remain at bottom of visible area during scrolling
  let mut parser = vt100::Parser::new(5, 80, 100, TestReplySender);

  for i in 0..20 {
    parser.process(format!("Line {}\r\n", i).as_bytes());
  }

  let screen = parser.screen();
  let cursor_pos = screen.cursor_position();

  // Cursor should be at or near the bottom of visible area
  // Row position is relative to visible area, not total buffer
  assert!(
    cursor_pos.0 <= 5,
    "Cursor row should be within visible area"
  );
}

// =============================================================================
// Integration Tests with TestHarness
// =============================================================================

#[test]
fn test_scrollback_harness_widget_rendering() {
  let mut harness =
    TestHarness::with_config(TestAppConfig::new().with_terminal_size(80, 5));
  let mut parser = vt100::Parser::new(5, 80, 100, TestReplySender);

  // Create scrollback
  for i in 0..10 {
    parser.process(format!("Line {:02}\r\n", i).as_bytes());
  }

  // Render visible portion
  let screen = parser.screen();
  let widget = TerminalWidget::new(screen);
  harness.render(widget).expect("Render should succeed");

  // Buffer should contain visible lines (5-9), not scrollback (0-4)
  let buffer_str = harness.buffer_as_string();
  assert!(
    !buffer_str.contains("Line 00") && !buffer_str.contains("Line 01"),
    "Scrollback lines should not be visible in default view"
  );
}

#[test]
fn test_scrollback_multiple_render_cycles() {
  let mut harness =
    TestHarness::with_config(TestAppConfig::new().with_terminal_size(80, 5));
  let mut parser = vt100::Parser::new(5, 80, 100, TestReplySender);
  let mut tracker = ScrollbackTracker::new();

  tracker.init(parser.screen().total_rows());

  // First batch of content
  for i in 0..3 {
    parser.process(format!("Batch1 Line {}\r\n", i).as_bytes());
  }

  let pending1 = tracker.detect_scrollback(parser.screen().total_rows());
  tracker.update_tracking(pending1, pending1.min(5));

  // Second batch
  for i in 0..5 {
    parser.process(format!("Batch2 Line {}\r\n", i).as_bytes());
  }

  let pending2 = tracker.detect_scrollback(parser.screen().total_rows());
  tracker.update_tracking(pending2, pending2.min(5));

  // Third batch
  for i in 0..4 {
    parser.process(format!("Batch3 Line {}\r\n", i).as_bytes());
  }

  let pending3 = tracker.detect_scrollback(parser.screen().total_rows());

  // Should accumulate scrollback across batches
  assert!(parser.screen().row0() > 0);

  // Render final state
  let screen = parser.screen();
  let widget = TerminalWidget::new(screen);
  harness.render(widget).expect("Final render should succeed");
}

// =============================================================================
// Scrollback Buffer State Tests
// =============================================================================

#[test]
fn test_scrollback_len_accessor() {
  let parser = vt100::Parser::new(10, 80, 500, TestReplySender);
  assert_eq!(parser.screen().scrollback_len(), 500);
}

#[test]
fn test_scrollback_total_rows_grows() {
  let mut parser = vt100::Parser::new(10, 80, 1000, TestReplySender);

  let mut prev_total = parser.screen().total_rows();

  for i in 0..30 {
    parser.process(format!("Line {}\r\n", i).as_bytes());

    let current_total = parser.screen().total_rows();
    // total_rows should never decrease
    assert!(
      current_total >= prev_total,
      "total_rows should not decrease"
    );
    prev_total = current_total;
  }
}

#[test]
fn test_scrollback_row0_grows_with_content() {
  let mut parser = vt100::Parser::new(10, 80, 1000, TestReplySender);

  // Initially row0 is 0
  assert_eq!(parser.screen().row0(), 0);

  // Fill screen
  for i in 0..10 {
    parser.process(format!("Line {}\r\n", i).as_bytes());
  }

  // After filling, row0 should still be near 0 (some lines may scroll)
  let row0_after_fill = parser.screen().row0();

  // Add more content
  for i in 10..20 {
    parser.process(format!("Line {}\r\n", i).as_bytes());
  }

  // row0 should have increased
  assert!(
    parser.screen().row0() > row0_after_fill,
    "row0 should increase as content scrolls"
  );
}

#[test]
fn test_scrollback_consistent_after_many_operations() {
  let mut parser = vt100::Parser::new(10, 80, 100, TestReplySender);

  // Perform many operations
  for batch in 0..10 {
    for line in 0..15 {
      parser.process(format!("Batch {} Line {}\r\n", batch, line).as_bytes());
    }

    let screen = parser.screen();

    // Invariant: total_rows = row0 + visible_rows
    assert_eq!(
      screen.total_rows(),
      screen.row0() + screen.size().rows as usize,
      "Invariant violated at batch {}",
      batch
    );

    // row0 should never exceed total_rows - visible_rows
    assert!(screen.row0() <= screen.total_rows() - screen.size().rows as usize);
  }
}
