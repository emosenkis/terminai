// Scrollback injection module
//
// This module handles detecting when content scrolls off the VT100 terminal's
// visible area and needs to be pushed to the host terminal's native scrollback.
//
// The mechanism works as follows:
// 1. Track the last known total_rows from the VT100 screen
// 2. When total_rows increases, content has scrolled into VT100's scrollback
// 3. Calculate how many lines scrolled and which rows to extract
// 4. Render those rows to the host terminal buffer
// 5. Signal the host terminal to scroll up (pushing lines to native scrollback)

use crate::vt100::{self, TermReplySender};
use tui::style::Modifier;

/// Tracks scrollback state between render cycles.
///
/// This struct maintains the state needed to detect when new content
/// has scrolled off the visible area of the VT100 terminal.
#[derive(Debug, Clone)]
pub struct ScrollbackTracker {
  /// Last known total row count from the VT100 screen
  last_total_rows: usize,
  /// Whether there are still pending scrollback lines to process
  /// (when more lines scrolled than can be processed in one frame)
  has_pending_scrollback: bool,
}

/// Result of detecting scrollback changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrollbackDetection {
  /// Number of new lines that have scrolled off since last check
  pub num_pending_lines: usize,
  /// Number of rows to actually scroll this frame (limited by screen height)
  pub rows_to_scroll: usize,
  /// Whether there will still be pending scrollback after this frame
  pub has_pending_after: bool,
}

/// Information needed to extract scrollback rows from the screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrollbackRange {
  /// Index in all_rows() to start extracting from
  pub start_index: usize,
  /// Number of rows to extract
  pub count: usize,
}

impl Default for ScrollbackTracker {
  fn default() -> Self {
    Self::new()
  }
}

impl ScrollbackTracker {
  /// Create a new scrollback tracker.
  pub fn new() -> Self {
    Self {
      last_total_rows: 0,
      has_pending_scrollback: false,
    }
  }

  /// Initialize the tracker with the current screen state.
  ///
  /// Call this when first setting up the terminal to sync the tracker
  /// with the current VT100 screen state.
  pub fn init(&mut self, total_rows: usize) {
    self.last_total_rows = total_rows;
    self.has_pending_scrollback = false;
  }

  /// Initialize the tracker from a VT100 screen.
  pub fn init_from_screen<T: TermReplySender>(
    &mut self,
    screen: &vt100::Screen<T>,
  ) {
    self.init(screen.total_rows());
  }

  /// Get the last known total rows count.
  pub fn last_total_rows(&self) -> usize {
    self.last_total_rows
  }

  /// Check if there are pending scrollback lines from a previous frame.
  pub fn has_pending_scrollback(&self) -> bool {
    self.has_pending_scrollback
  }

  /// Detect if new content has scrolled off the visible area.
  ///
  /// Compares the current total_rows with the last known value to detect
  /// how many new lines have entered scrollback.
  ///
  /// # Arguments
  /// * `current_total_rows` - Current total row count from the VT100 screen
  /// * `screen_height` - Height of the visible screen area (limits rows per frame)
  ///
  /// # Returns
  /// A `ScrollbackDetection` with information about pending scrollback.
  pub fn detect(
    &self,
    current_total_rows: usize,
    screen_height: usize,
  ) -> ScrollbackDetection {
    let num_pending_lines = if current_total_rows > self.last_total_rows {
      current_total_rows - self.last_total_rows
    } else {
      0
    };

    let rows_to_scroll = num_pending_lines.min(screen_height);
    let has_pending_after = rows_to_scroll < num_pending_lines;

    ScrollbackDetection {
      num_pending_lines,
      rows_to_scroll,
      has_pending_after,
    }
  }

  /// Detect scrollback from a VT100 screen.
  pub fn detect_from_screen<T: TermReplySender>(
    &self,
    screen: &vt100::Screen<T>,
    screen_height: usize,
  ) -> ScrollbackDetection {
    self.detect(screen.total_rows(), screen_height)
  }

  /// Update the tracker state after processing scrollback.
  ///
  /// Call this after rendering the scrollback lines to update
  /// the tracker's internal state.
  ///
  /// # Arguments
  /// * `detection` - The detection result from `detect()`
  pub fn update(&mut self, detection: &ScrollbackDetection) {
    self.last_total_rows += detection.rows_to_scroll;
    self.has_pending_scrollback = detection.has_pending_after;
  }

  /// Calculate the range of rows to extract from the screen's all_rows().
  ///
  /// The scrollback lines are located just before row0 in the all_rows iterator.
  /// This calculates the correct start index and count to extract.
  ///
  /// # Arguments
  /// * `row0` - The current row0 from the VT100 screen (where visible rows start)
  /// * `num_pending_lines` - Number of pending lines from detection
  /// * `rows_to_scroll` - Number of rows to actually scroll
  ///
  /// # Returns
  /// A `ScrollbackRange` indicating which rows to extract.
  pub fn calculate_range(
    row0: usize,
    num_pending_lines: usize,
    rows_to_scroll: usize,
  ) -> ScrollbackRange {
    // The lines that just scrolled off are at indices:
    // (row0 - num_pending_lines) through (row0 - 1)
    // But we only take rows_to_scroll of them
    let start_index = row0.saturating_sub(num_pending_lines);

    ScrollbackRange {
      start_index,
      count: rows_to_scroll,
    }
  }

  /// Calculate the range from a VT100 screen and detection result.
  pub fn calculate_range_from_screen<T: TermReplySender>(
    screen: &vt100::Screen<T>,
    detection: &ScrollbackDetection,
  ) -> ScrollbackRange {
    Self::calculate_range(
      screen.row0(),
      detection.num_pending_lines,
      detection.rows_to_scroll,
    )
  }
}

/// Render scrollback rows from a VT100 screen to a tui buffer.
///
/// This function extracts the scrollback rows identified by the range
/// and renders them into the tui buffer at the specified position.
/// This is the main integration point with the rendering pipeline.
///
/// # Arguments
/// * `screen` - The VT100 screen to extract scrollback from
/// * `range` - The range of rows to extract (from ScrollbackTracker::calculate_range)
/// * `buf` - The tui buffer to render into
/// * `start_x` - X coordinate to start rendering at
/// * `start_y` - Y coordinate to start rendering at
/// * `max_width` - Maximum width to render
///
/// # Returns
/// The number of lines actually rendered.
pub fn render_scrollback_to_buffer<T: TermReplySender>(
  screen: &vt100::Screen<T>,
  range: &ScrollbackRange,
  buf: &mut tui::buffer::Buffer,
  start_x: u16,
  start_y: u16,
  max_width: u16,
) -> usize {
  let screen_cols = screen.size().cols;
  let effective_width = max_width.min(screen_cols);

  let mut line_idx = 0;
  for row in screen.all_rows().skip(range.start_index).take(range.count) {
    let row_cols = row.cols().min(effective_width);
    for col in 0..row_cols {
      if let Some(cell) = row.get(col) {
        if let Some(buf_cell) =
          buf.cell_mut((start_x + col, start_y + line_idx as u16))
        {
          *buf_cell = cell.to_tui();
          if !cell.has_contents() {
            buf_cell.modifier |= Modifier::EMPTY;
          }
        }
      }
    }
    line_idx += 1;
  }
  line_idx
}

pub fn render_rows_to_buffer(
  rows: &[vt100::Row],
  buf: &mut tui::buffer::Buffer,
  start_x: u16,
  start_y: u16,
  max_width: u16,
) -> usize {
  let mut line_idx = 0;
  for row in rows {
    let row_cols = row.cols().min(max_width);
    for col in 0..row_cols {
      if let Some(buf_cell) =
        buf.cell_mut((start_x + col, start_y + line_idx as u16))
      {
        if let Some(cell) = row.get(col) {
          *buf_cell = cell.to_tui();
          if !cell.has_contents() {
            buf_cell.modifier |= Modifier::EMPTY;
          }
        }
      }
    }
    line_idx += 1;
  }
  line_idx
}

pub fn process_pending_native_scrollback<T: TermReplySender + Clone>(
  parser: &mut vt100::Parser<T>,
  buf: &mut tui::buffer::Buffer,
  area: tui::layout::Rect,
) -> u16 {
  let rows_to_scroll = parser
    .pending_native_scrollback_len()
    .min(area.height as usize);
  if rows_to_scroll == 0 {
    return 0;
  }

  let rows = parser.drain_pending_native_scrollback(rows_to_scroll);
  render_rows_to_buffer(&rows, buf, area.x, area.y, area.width);

  rows.len() as u16
}

pub fn drain_pending_native_scrollback_snapshot<T: TermReplySender + Clone>(
  parser: &mut vt100::Parser<T>,
  width: u16,
) -> Option<(Vec<tui::buffer::Cell>, usize)> {
  let rows_to_scroll = parser.pending_native_scrollback_len();
  if rows_to_scroll == 0 || width == 0 {
    return None;
  }

  let rows = parser.drain_pending_native_scrollback(rows_to_scroll);
  let mut content = Vec::with_capacity(rows.len() * width as usize);

  for row in &rows {
    for col in 0..width {
      let mut cell = row
        .get(col)
        .map(crate::vt100::Cell::to_tui)
        .unwrap_or_default();
      if !row.get(col).is_some_and(crate::vt100::Cell::has_contents) {
        cell.modifier |= Modifier::EMPTY;
      }
      content.push(cell);
    }
  }

  Some((content, rows.len()))
}

/// High-level function to process scrollback injection in a single call.
///
/// This combines detection, range calculation, and rendering into one
/// convenient function that mirrors the logic in terminai.rs's render function.
///
/// # Arguments
/// * `tracker` - The scrollback tracker (will be updated)
/// * `screen` - The VT100 screen to extract scrollback from
/// * `buf` - The tui buffer to render into
/// * `area` - The area to render into
///
/// # Returns
/// The number of lines that need to be scrolled up (for frame.set_scroll_up)
pub fn process_scrollback<T: TermReplySender>(
  tracker: &mut ScrollbackTracker,
  screen: &vt100::Screen<T>,
  buf: &mut tui::buffer::Buffer,
  area: tui::layout::Rect,
) -> u16 {
  let detection = tracker.detect_from_screen(screen, area.height as usize);

  if detection.num_pending_lines == 0 {
    return 0;
  }

  let range =
    ScrollbackTracker::calculate_range_from_screen(screen, &detection);

  render_scrollback_to_buffer(screen, &range, buf, area.x, area.y, area.width);

  tracker.update(&detection);

  detection.rows_to_scroll as u16
}

#[cfg(test)]
mod tests {
  use super::*;

  // ==========================================================================
  // ScrollbackTracker Unit Tests
  // ==========================================================================

  #[test]
  fn test_tracker_new() {
    let tracker = ScrollbackTracker::new();
    assert_eq!(tracker.last_total_rows(), 0);
    assert!(!tracker.has_pending_scrollback());
  }

  #[test]
  fn test_tracker_init() {
    let mut tracker = ScrollbackTracker::new();
    tracker.init(100);
    assert_eq!(tracker.last_total_rows(), 100);
    assert!(!tracker.has_pending_scrollback());
  }

  #[test]
  fn test_detect_no_change() {
    let mut tracker = ScrollbackTracker::new();
    tracker.init(10);

    let detection = tracker.detect(10, 5);
    assert_eq!(detection.num_pending_lines, 0);
    assert_eq!(detection.rows_to_scroll, 0);
    assert!(!detection.has_pending_after);
  }

  #[test]
  fn test_detect_decrease_no_scrollback() {
    // If total_rows decreases (e.g., screen resize), no scrollback
    let mut tracker = ScrollbackTracker::new();
    tracker.init(20);

    let detection = tracker.detect(15, 10);
    assert_eq!(detection.num_pending_lines, 0);
    assert_eq!(detection.rows_to_scroll, 0);
  }

  #[test]
  fn test_detect_single_line() {
    let mut tracker = ScrollbackTracker::new();
    tracker.init(10);

    let detection = tracker.detect(11, 10);
    assert_eq!(detection.num_pending_lines, 1);
    assert_eq!(detection.rows_to_scroll, 1);
    assert!(!detection.has_pending_after);
  }

  #[test]
  fn test_detect_multiple_lines_within_screen() {
    let mut tracker = ScrollbackTracker::new();
    tracker.init(10);

    let detection = tracker.detect(15, 10);
    assert_eq!(detection.num_pending_lines, 5);
    assert_eq!(detection.rows_to_scroll, 5);
    assert!(!detection.has_pending_after);
  }

  #[test]
  fn test_detect_more_than_screen_height() {
    let mut tracker = ScrollbackTracker::new();
    tracker.init(10);

    // 20 new lines but screen height is only 5
    let detection = tracker.detect(30, 5);
    assert_eq!(detection.num_pending_lines, 20);
    assert_eq!(detection.rows_to_scroll, 5); // Limited by screen height
    assert!(detection.has_pending_after);
  }

  #[test]
  fn test_update_advances_state() {
    let mut tracker = ScrollbackTracker::new();
    tracker.init(10);

    let detection = tracker.detect(15, 10);
    tracker.update(&detection);

    assert_eq!(tracker.last_total_rows(), 15);
    assert!(!tracker.has_pending_scrollback());
  }

  #[test]
  fn test_update_with_pending() {
    let mut tracker = ScrollbackTracker::new();
    tracker.init(10);

    // More lines than can be processed
    let detection = tracker.detect(30, 5);
    tracker.update(&detection);

    assert_eq!(tracker.last_total_rows(), 15); // 10 + 5
    assert!(tracker.has_pending_scrollback());
  }

  #[test]
  fn test_detect_after_partial_update() {
    let mut tracker = ScrollbackTracker::new();
    tracker.init(10);

    // First frame: 20 new lines, process 5
    let detection1 = tracker.detect(30, 5);
    assert_eq!(detection1.num_pending_lines, 20);
    tracker.update(&detection1);

    // Second frame: should see remaining 15 lines
    let detection2 = tracker.detect(30, 5);
    assert_eq!(detection2.num_pending_lines, 15);
    assert_eq!(detection2.rows_to_scroll, 5);
    assert!(detection2.has_pending_after);
  }

  // ==========================================================================
  // ScrollbackRange Tests
  // ==========================================================================

  #[test]
  fn test_calculate_range_basic() {
    let range = ScrollbackTracker::calculate_range(10, 5, 5);
    assert_eq!(range.start_index, 5); // 10 - 5
    assert_eq!(range.count, 5);
  }

  #[test]
  fn test_calculate_range_partial() {
    // Only processing 3 of 5 pending lines
    let range = ScrollbackTracker::calculate_range(10, 5, 3);
    assert_eq!(range.start_index, 5); // Start is same
    assert_eq!(range.count, 3); // But we only take 3
  }

  #[test]
  fn test_calculate_range_saturating() {
    // Edge case: num_pending > row0 (shouldn't happen normally)
    let range = ScrollbackTracker::calculate_range(3, 10, 3);
    assert_eq!(range.start_index, 0); // Saturates to 0
    assert_eq!(range.count, 3);
  }

  #[test]
  fn test_calculate_range_zero_pending() {
    let range = ScrollbackTracker::calculate_range(10, 0, 0);
    assert_eq!(range.start_index, 10);
    assert_eq!(range.count, 0);
  }

  // ==========================================================================
  // Multi-Frame Simulation Tests
  // ==========================================================================

  #[test]
  fn test_continuous_scrolling() {
    let mut tracker = ScrollbackTracker::new();
    let screen_height = 10;
    let mut simulated_total_rows = 10;

    tracker.init(simulated_total_rows);

    // Simulate 5 frames of content being added
    for frame in 0..5 {
      // Add 3 new lines each frame
      simulated_total_rows += 3;

      let detection = tracker.detect(simulated_total_rows, screen_height);
      assert_eq!(
        detection.num_pending_lines, 3,
        "Frame {}: expected 3 pending lines",
        frame
      );

      tracker.update(&detection);
      assert_eq!(
        tracker.last_total_rows(),
        simulated_total_rows,
        "Frame {}: tracker should be in sync",
        frame
      );
    }
  }

  #[test]
  fn test_burst_then_idle() {
    let mut tracker = ScrollbackTracker::new();
    tracker.init(10);

    // Burst: 50 new lines
    let detection = tracker.detect(60, 10);
    assert_eq!(detection.num_pending_lines, 50);
    assert_eq!(detection.rows_to_scroll, 10);
    tracker.update(&detection);

    // Process remaining in batches
    while tracker.has_pending_scrollback() {
      let detection = tracker.detect(60, 10);
      tracker.update(&detection);
    }

    assert_eq!(tracker.last_total_rows(), 60);

    // Idle: no new content
    let detection = tracker.detect(60, 10);
    assert_eq!(detection.num_pending_lines, 0);
  }
}
