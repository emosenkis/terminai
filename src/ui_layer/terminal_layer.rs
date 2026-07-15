// Terminal layer - renders the VT100 terminal emulator

use tui::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::vt100::{self, TermReplySender};

/// Widget that renders a VT100 screen to a tui buffer
pub struct TerminalWidget<'a, R: TermReplySender + Clone> {
  screen: &'a vt100::Screen<R>,
  row_offset: u16,
}

impl<'a, R: TermReplySender + Clone> TerminalWidget<'a, R> {
  pub fn new(screen: &'a vt100::Screen<R>) -> Self {
    Self {
      screen,
      row_offset: 0,
    }
  }

  pub fn with_offset(screen: &'a vt100::Screen<R>, row_offset: u16) -> Self {
    Self { screen, row_offset }
  }
}

impl<R: TermReplySender + Clone> Widget for TerminalWidget<'_, R> {
  fn render(self, area: Rect, buf: &mut Buffer) {
    // Render each cell from the VT100 screen to the tui buffer
    // Pattern borrowed from mprocs' ui_term.rs
    for row in 0..area.height {
      let source_row = row + self.row_offset;
      for col in 0..area.width {
        let pos = tui::layout::Position {
          x: area.x + col,
          y: area.y + row,
        };

        if let Some(to_cell) = buf.cell_mut(pos) {
          // Apply row offset to shift viewport (for AI overlay)
          if let Some(cell) = self.screen.cell(source_row, col) {
            // Convert VT100 cell to tui cell (using mprocs' conversion)
            *to_cell = cell.to_tui();
            if !cell.has_contents() {
              to_cell.set_char(' ');
            }
          } else {
            // Out of bounds (offset pushed us past screen size)
            to_cell.set_char(' ');
          }
        }
      }

      if self.screen.row_wrapped(source_row) {
        for col in (0..area.width).rev() {
          if self
            .screen
            .cell(source_row, col)
            .is_some_and(vt100::Cell::has_contents)
          {
            if let Some(cell) = buf.cell_mut((area.x + col, area.y + row)) {
              cell.set_soft_wrap(true);
            }
            break;
          }
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::vt100::Parser;
  use std::sync::{Arc, RwLock};

  // Simple test reply sender
  #[derive(Clone)]
  struct TestReplySender;

  impl TermReplySender for TestReplySender {
    fn reply(&self, _reply: compact_str::CompactString) {
      // No-op for testing
    }
  }

  fn create_test_vt() -> Arc<RwLock<Parser<TestReplySender>>> {
    // Parser::new(rows, cols, scrollback_len, reply_sender)
    Arc::new(RwLock::new(Parser::new(24, 80, 1000, TestReplySender)))
  }

  #[test]
  fn test_terminal_widget_render() {
    let vt = create_test_vt();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);

    if let Ok(parser) = vt.read() {
      let screen = parser.screen();
      let widget = TerminalWidget::new(screen);
      widget.render(area, &mut buf);
    }
  }

  #[test]
  fn test_terminal_widget_with_offset() {
    let vt = create_test_vt();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);

    if let Ok(parser) = vt.read() {
      let screen = parser.screen();
      let widget = TerminalWidget::with_offset(screen, 5);
      widget.render(area, &mut buf);
    }
  }

  #[test]
  fn test_terminal_widget_with_content() {
    let vt = create_test_vt();

    // Write some content to the parser
    {
      let mut parser = vt.write().unwrap();
      parser.process(b"Hello, World!\r\n");
      parser.process(b"Second line\r\n");
    }

    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);

    if let Ok(parser) = vt.read() {
      let screen = parser.screen();
      let widget = TerminalWidget::new(screen);
      widget.render(area, &mut buf);
    }

    // Verify content is rendered
    let cell = buf.cell((0, 0)).unwrap();
    assert_eq!(cell.symbol(), "H");
  }
}
