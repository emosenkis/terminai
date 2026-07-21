use std::sync::{Arc, Mutex};

use crate::vt100::TermReplySender;

/// A parser for terminal output which produces an in-memory representation of
/// the terminal contents.
pub struct Parser<Reply: TermReplySender + Clone> {
  parser: Arc<Mutex<termwiz::escape::parser::Parser>>,
  pub screen: crate::vt100::screen::Screen<Reply>,
}

impl<Reply: TermReplySender + Clone> Parser<Reply> {
  /// Creates a new terminal parser of the given size and with the given
  /// amount of scrollback.
  #[must_use]
  pub fn new(
    rows: u16,
    cols: u16,
    scrollback_len: usize,
    reply_sender: Reply,
  ) -> Self {
    let parser = Arc::new(Mutex::new(termwiz::escape::parser::Parser::new()));
    Self {
      parser,
      screen: crate::vt100::screen::Screen::new(
        crate::vt100::grid::Size { rows, cols },
        scrollback_len,
        reply_sender,
      ),
    }
  }

  /// Processes the contents of the given byte string, and updates the
  /// in-memory terminal state.
  pub fn process(&mut self, bytes: &[u8]) {
    self.parser.lock().unwrap().parse(bytes, |action| {
      self.screen.handle_action(action);
    });
  }

  /// Resizes the terminal.
  pub fn set_size(&mut self, rows: u16, cols: u16) {
    // Terminal backends can briefly report a zero-sized window while it is
    // minimized. A grid requires at least one row and one column.
    if rows == 0 || cols == 0 {
      return;
    }
    self.screen.set_size(rows, cols);
  }

  /// Scrolls to the given position in the scrollback.
  ///
  /// This position indicates the offset from the top of the screen, and
  /// should be `0` to put the normal screen in view.
  ///
  /// This affects the return values of methods called on `parser.screen()`:
  /// for instance, `parser.screen().cell(0, 0)` will return the top left
  /// corner of the screen after taking the scrollback offset into account.
  /// It does not affect `parser.process()` at all.
  ///
  /// The value given will be clamped to the actual size of the scrollback.
  pub fn set_scrollback(&mut self, rows: usize) {
    self.screen.set_scrollback(rows);
  }

  pub fn pending_native_scrollback_len(&self) -> usize {
    self.screen.pending_native_scrollback_len()
  }

  pub fn drain_pending_native_scrollback(
    &mut self,
    count: usize,
  ) -> Vec<crate::vt100::row::Row> {
    self.screen.drain_pending_native_scrollback(count)
  }

  /// Discards internal scrollback while preserving the visible screen.
  pub fn clear_scrollback(&mut self) {
    self.screen.clear_scrollback();
  }

  /// Returns a reference to a `Screen` object containing the terminal
  /// state.
  #[must_use]
  pub fn screen(&self) -> &crate::vt100::screen::Screen<Reply> {
    &self.screen
  }
}

impl<Reply: TermReplySender + Clone> std::io::Write for Parser<Reply> {
  fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
    self.process(buf);
    Ok(buf.len())
  }

  fn flush(&mut self) -> std::io::Result<()> {
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Clone)]
  struct TestReplySender;

  impl TermReplySender for TestReplySender {
    fn reply(&self, _reply: compact_str::CompactString) {}
  }

  #[test]
  fn zero_column_resize_does_not_leave_the_parser_unwritable() {
    let mut parser = Parser::new(24, 80, 0, TestReplySender);

    parser.set_size(24, 0);
    parser.process(b"output after a zero-column resize");

    assert_eq!(parser.screen().size().cols, 80);
    assert_eq!(parser.screen().cell(0, 0).unwrap().contents(), "o");
  }

  #[test]
  fn clear_scrollback_preserves_visible_screen() {
    let mut parser = Parser::new(3, 8, 100, TestReplySender);
    for i in 0..8 {
      parser.process(format!("line-{i}\r\n").as_bytes());
    }
    let visible_before = parser
      .screen()
      .drawing_rows()
      .map(|row| {
        let mut text = String::new();
        row.write_contents(&mut text, 0, row.cols(), false);
        text
      })
      .collect::<Vec<_>>();
    assert!(parser.screen().total_rows() > 3);
    assert!(parser.pending_native_scrollback_len() > 0);

    parser.clear_scrollback();

    let visible_after = parser
      .screen()
      .drawing_rows()
      .map(|row| {
        let mut text = String::new();
        row.write_contents(&mut text, 0, row.cols(), false);
        text
      })
      .collect::<Vec<_>>();
    assert_eq!(visible_after, visible_before);
    assert_eq!(parser.screen().total_rows(), 3);
    assert_eq!(parser.screen().scrollback(), 0);
    assert_eq!(parser.pending_native_scrollback_len(), 0);
  }
}
