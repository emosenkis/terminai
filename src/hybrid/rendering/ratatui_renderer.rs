//! Ratatui renderer for terminal content and modals
//!
//! This module provides rendering of terminal content via ratatui, including
//! modal overlays.

use std::io::{self, Stdout};
use tui::{Frame, Terminal, backend::CrosstermBackend, layout::Rect};

use super::modal::ModalState;
use crate::hybrid::terminal::TerminalContent;

/// Handles ratatui rendering when in modal or guest-alt-buffer modes
pub struct RatatuiRenderer {
  /// The ratatui terminal
  terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl RatatuiRenderer {
  /// Create a new ratatui renderer
  ///
  /// Note: This assumes the stdout is already in raw mode and alternate screen
  /// if needed. The caller (HybridTerminal) is responsible for managing that.
  pub fn new(stdout: Stdout) -> io::Result<Self> {
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(Self { terminal })
  }

  /// Render a frame with terminal content and optional modal
  ///
  /// This is the main entry point for rendering. It renders the terminal
  /// content as the background and optionally overlays a modal on top.
  pub fn render_frame(
    &mut self,
    terminal_content: &TerminalContent,
    modal_state: Option<&mut ModalState>,
  ) -> io::Result<()> {
    self.terminal.draw(|frame| {
      let area = frame.area();

      // Render the terminal content as background
      Self::render_terminal_content(frame, terminal_content, area);

      // Render modal on top if visible
      if let Some(modal) = modal_state {
        modal.render(frame, area);
      }
    })?;

    Ok(())
  }

  /// Render terminal content to the frame
  ///
  /// This copies the cells from the shadow terminal's content grid
  /// into the ratatui frame buffer.
  fn render_terminal_content(
    frame: &mut Frame,
    content: &TerminalContent,
    area: Rect,
  ) {
    let buf = frame.buffer_mut();

    // Copy cells from content to frame buffer
    for (row_idx, row) in content.cells.iter().enumerate() {
      let row_idx = row_idx as u16;
      if row_idx >= area.height {
        break;
      }

      for (col_idx, cell) in row.iter().enumerate() {
        let col_idx = col_idx as u16;
        if col_idx >= area.width {
          break;
        }

        let x = area.x + col_idx;
        let y = area.y + row_idx;

        // Copy cell content and style
        if let Some(buf_cell) = buf.cell_mut((x, y)) {
          *buf_cell = cell.clone();
        }
      }
    }

    // Set cursor position if visible
    if content.cursor_visible {
      let (cursor_col, cursor_row) = content.cursor;
      if cursor_col < area.width && cursor_row < area.height {
        frame.set_cursor_position((area.x + cursor_col, area.y + cursor_row));
      }
    }
  }

  /// Get the current terminal size
  pub fn size(&self) -> io::Result<(u16, u16)> {
    let size = self.terminal.size()?;
    Ok((size.width, size.height))
  }

  /// Force a full redraw
  pub fn clear(&mut self) -> io::Result<()> {
    self.terminal.clear()
  }

  /// Get a mutable reference to the underlying terminal
  ///
  /// This can be used for advanced operations like autoresizing.
  pub fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
    &mut self.terminal
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  // Note: We can't easily test rendering without a real terminal,
  // but we can test the basic structure

  #[test]
  fn test_render_empty_content() {
    // This test just verifies the code compiles and has the right structure
    // Actual rendering tests would require a mock terminal
    let content = TerminalContent::empty(80, 24);
    assert_eq!(content.width(), 80);
    assert_eq!(content.height(), 24);
  }
}
