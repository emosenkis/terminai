// Terminal layer - renders the VT100 terminal emulator
// This is always the bottom layer in the stack

use anyhow::Result;
use crossterm::event::Event;
use std::sync::{Arc, RwLock};
use tui::{buffer::Buffer, layout::Rect, widgets::Widget};

use super::{LayerEventOutcome, UILayer};
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
      for col in 0..area.width {
        let pos = tui::layout::Position {
          x: area.x + col,
          y: area.y + row,
        };

        if let Some(to_cell) = buf.cell_mut(pos) {
          // Apply row offset to shift viewport (for AI overlay)
          if let Some(cell) = self.screen.cell(row + self.row_offset, col) {
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
    }
  }
}

/// Terminal layer - the base layer that renders the VT100 terminal
pub struct TerminalLayer<R: TermReplySender + Clone> {
  /// Reference to the VT100 parser
  vt: Arc<RwLock<vt100::Parser<R>>>,
  /// Row offset for rendering (used when AI overlay is visible at bottom)
  row_offset: u16,
  /// Cached cursor position
  cursor_pos: Option<(u16, u16)>,
  /// Whether to show the cursor
  show_cursor: bool,
  /// Render area (cached for cursor calculation)
  render_area: Rect,
}

impl<R: TermReplySender + Clone> TerminalLayer<R> {
  pub fn new(vt: Arc<RwLock<vt100::Parser<R>>>) -> Self {
    Self {
      vt,
      row_offset: 0,
      cursor_pos: None,
      show_cursor: true,
      render_area: Rect::default(),
    }
  }

  /// Set the row offset for rendering (shifts viewport up to make room for overlay)
  pub fn set_row_offset(&mut self, offset: u16) {
    self.row_offset = offset;
  }

  /// Get the current row offset
  pub fn row_offset(&self) -> u16 {
    self.row_offset
  }

  /// Get a reference to the VT parser
  pub fn vt(&self) -> &Arc<RwLock<vt100::Parser<R>>> {
    &self.vt
  }
}

impl<R: TermReplySender + Clone + 'static> UILayer for TerminalLayer<R> {
  fn is_visible(&self) -> bool {
    // Terminal layer is always visible
    true
  }

  fn render(&mut self, area: Rect, buf: &mut Buffer) {
    self.render_area = area;

    if let Ok(vt) = self.vt.read() {
      let screen = vt.screen();
      let widget = TerminalWidget::with_offset(screen, self.row_offset);
      widget.render(area, buf);

      // Cache cursor info for screen_cursor()
      if !screen.hide_cursor() {
        let cursor = screen.cursor_position();
        // Adjust for row offset
        if cursor.0 >= self.row_offset {
          self.cursor_pos =
            Some((area.x + cursor.1, area.y + cursor.0 - self.row_offset));
          self.show_cursor = true;
        } else {
          // Cursor is above visible area due to offset
          self.cursor_pos = None;
          self.show_cursor = false;
        }
      } else {
        self.cursor_pos = None;
        self.show_cursor = false;
      }
    } else {
      log::warn!("Failed to acquire VT read lock in terminal layer");
      self.cursor_pos = None;
      self.show_cursor = false;
    }
  }

  fn handle_event(&mut self, _event: &Event) -> Result<LayerEventOutcome> {
    // Terminal layer doesn't handle events directly
    // Events are handled by the main app which routes to Shell
    Ok(LayerEventOutcome::Continue)
  }

  fn screen_cursor(&self) -> Option<(u16, u16)> {
    if self.show_cursor {
      self.cursor_pos
    } else {
      None
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::vt100::Parser;

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
  fn test_terminal_layer_always_visible() {
    let vt = create_test_vt();
    let layer = TerminalLayer::new(vt);
    assert!(layer.is_visible());
  }

  #[test]
  fn test_terminal_layer_row_offset() {
    let vt = create_test_vt();
    let mut layer = TerminalLayer::new(vt);

    assert_eq!(layer.row_offset(), 0);

    layer.set_row_offset(10);
    assert_eq!(layer.row_offset(), 10);
  }

  #[test]
  fn test_terminal_layer_render() {
    let vt = create_test_vt();
    let mut layer = TerminalLayer::new(vt);

    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);

    // Should not panic
    layer.render(area, &mut buf);
  }

  #[test]
  fn test_terminal_layer_event_continues() {
    let vt = create_test_vt();
    let mut layer = TerminalLayer::new(vt);

    let event = Event::Key(crossterm::event::KeyEvent::new(
      crossterm::event::KeyCode::Char('a'),
      crossterm::event::KeyModifiers::empty(),
    ));

    let result = layer.handle_event(&event).unwrap();
    assert_eq!(result, LayerEventOutcome::Continue);
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
  fn test_terminal_layer_cursor_hidden() {
    let vt = create_test_vt();
    let mut layer = TerminalLayer::new(vt);

    // Before render, cursor should be None
    assert_eq!(layer.screen_cursor(), None);
  }

  #[test]
  fn test_terminal_layer_with_content() {
    let vt = create_test_vt();

    // Write some content to the parser
    {
      let mut parser = vt.write().unwrap();
      parser.process(b"Hello, World!\r\n");
      parser.process(b"Second line\r\n");
    }

    let mut layer = TerminalLayer::new(vt);
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);

    layer.render(area, &mut buf);

    // Verify content is rendered
    let cell = buf.cell((0, 0)).unwrap();
    assert_eq!(cell.symbol(), "H");
  }
}
