//! Terminal content representation for rendering
//!
//! This module defines the data structures used to represent terminal content
//! that can be rendered via ratatui. It provides conversion from vt100 cells
//! to ratatui cells.

use tui::{
  buffer::Cell as RatatuiCell,
  style::{Color, Modifier, Style},
};

use crate::vt100::{self, Cell as Vt100Cell};

/// Terminal content ready for ratatui rendering
///
/// This structure represents a snapshot of the terminal screen that can be
/// efficiently rendered via ratatui.
#[derive(Debug, Clone)]
pub struct TerminalContent {
  /// Grid of cells (rows x cols)
  pub cells: Vec<Vec<RatatuiCell>>,

  /// Cursor position (col, row)
  pub cursor: (u16, u16),

  /// Whether the cursor should be visible
  pub cursor_visible: bool,

  /// Terminal dimensions
  pub size: (u16, u16), // (cols, rows)
}

impl TerminalContent {
  /// Create an empty terminal content of given size
  pub fn empty(cols: u16, rows: u16) -> Self {
    let cells = (0..rows)
      .map(|_| (0..cols).map(|_| RatatuiCell::default()).collect())
      .collect();

    Self {
      cells,
      cursor: (0, 0),
      cursor_visible: true,
      size: (cols, rows),
    }
  }

  /// Get the width in columns
  pub fn width(&self) -> u16 {
    self.size.0
  }

  /// Get the height in rows
  pub fn height(&self) -> u16 {
    self.size.1
  }
}

/// Map a vt100 color to a ratatui color
pub fn map_color(vt100_color: vt100::Color) -> Color {
  match vt100_color {
    vt100::Color::Default => Color::Reset,
    vt100::Color::Idx(0) => Color::Black,
    vt100::Color::Idx(1) => Color::Red,
    vt100::Color::Idx(2) => Color::Green,
    vt100::Color::Idx(3) => Color::Yellow,
    vt100::Color::Idx(4) => Color::Blue,
    vt100::Color::Idx(5) => Color::Magenta,
    vt100::Color::Idx(6) => Color::Cyan,
    vt100::Color::Idx(7) => Color::Gray,
    vt100::Color::Idx(8) => Color::DarkGray,
    vt100::Color::Idx(9) => Color::LightRed,
    vt100::Color::Idx(10) => Color::LightGreen,
    vt100::Color::Idx(11) => Color::LightYellow,
    vt100::Color::Idx(12) => Color::LightBlue,
    vt100::Color::Idx(13) => Color::LightMagenta,
    vt100::Color::Idx(14) => Color::LightCyan,
    vt100::Color::Idx(15) => Color::White,
    vt100::Color::Idx(n) => Color::Indexed(n),
    vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
  }
}

/// Map a vt100 cell to a ratatui cell
pub fn map_cell(vt100_cell: Option<&Vt100Cell>) -> RatatuiCell {
  let mut cell = RatatuiCell::default();

  if let Some(vt_cell) = vt100_cell {
    // Set character
    let contents = vt_cell.contents();
    cell.set_char(contents.chars().next().unwrap_or(' '));

    // Set style
    let mut style = Style::default();
    style = style.fg(map_color(vt_cell.fgcolor()));
    style = style.bg(map_color(vt_cell.bgcolor()));

    // Set modifiers
    let mut modifier = Modifier::empty();
    if vt_cell.bold() {
      modifier |= Modifier::BOLD;
    }
    if vt_cell.italic() {
      modifier |= Modifier::ITALIC;
    }
    if vt_cell.underline() {
      modifier |= Modifier::UNDERLINED;
    }
    if vt_cell.inverse() {
      modifier |= Modifier::REVERSED;
    }
    style = style.add_modifier(modifier);

    cell.set_style(style);
  }

  cell
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_empty_content() {
    let content = TerminalContent::empty(80, 24);
    assert_eq!(content.width(), 80);
    assert_eq!(content.height(), 24);
    assert_eq!(content.cells.len(), 24);
    assert_eq!(content.cells[0].len(), 80);
    assert_eq!(content.cursor, (0, 0));
    assert!(content.cursor_visible);
  }

  #[test]
  fn test_color_mapping() {
    assert_eq!(map_color(vt100::Color::Default), Color::Reset);
    assert_eq!(map_color(vt100::Color::Idx(0)), Color::Black);
    assert_eq!(map_color(vt100::Color::Idx(1)), Color::Red);
    assert_eq!(map_color(vt100::Color::Idx(255)), Color::Indexed(255));
    assert_eq!(
      map_color(vt100::Color::Rgb(255, 128, 64)),
      Color::Rgb(255, 128, 64)
    );
  }

  #[test]
  fn test_map_empty_cell() {
    let cell = map_cell(None);
    assert_eq!(cell.symbol(), " ");
  }
}
