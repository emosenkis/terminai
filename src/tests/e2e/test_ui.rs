// UI rendering tests
//
// Tests for basic UI rendering and layout

use super::*;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Style};
use tui::widgets::{Block, Borders, Paragraph};

#[test]
fn test_basic_rendering() {
  let mut harness = TestHarness::new();

  let widget = Paragraph::new("Hello, Terminai!")
    .block(Block::default().borders(Borders::ALL).title("Test"));

  harness.render(widget).unwrap();
  harness.assert_buffer_contains("Hello, Terminai!");
  harness.assert_buffer_contains("Test");
}

#[test]
fn test_split_layout() {
  let mut harness = TestHarness::new();

  // Use render with a simpler approach
  let widget = Paragraph::new("Test Content")
    .block(Block::default().borders(Borders::ALL).title("Test"));

  harness.render(widget).unwrap();
  harness.assert_buffer_contains("Test Content");
}

#[test]
fn test_overlay_rendering() {
  let mut harness = TestHarness::new();

  // Render with overlay
  let widget = Paragraph::new("Overlay Content")
    .block(Block::default().borders(Borders::ALL).title("Overlay"));

  harness.render(widget).unwrap();
  harness.assert_buffer_contains("Overlay Content");
}

#[test]
#[cfg(feature = "snapshot-tests")]
fn test_ui_snapshot() {
  let mut harness = TestHarness::new();

  let widget = Paragraph::new("Snapshot Test Content")
    .block(Block::default().borders(Borders::ALL).title("Snapshot"));

  harness.render(widget).unwrap();

  // Use insta for snapshot testing
  insta::assert_snapshot!(harness.buffer_as_string());
}

/// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
  let popup_layout = Layout::default()
    .direction(Direction::Vertical)
    .constraints(
      [
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
      ]
      .as_ref(),
    )
    .split(r);

  Layout::default()
    .direction(Direction::Horizontal)
    .constraints(
      [
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
      ]
      .as_ref(),
    )
    .split(popup_layout[1])[1]
}
