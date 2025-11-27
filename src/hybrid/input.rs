//! Input handling for the hybrid terminal
//!
//! This module handles keyboard input and routing it either to the modal
//! or to the PTY based on the current mode.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Convert a crossterm key event to terminal escape sequence bytes
///
/// This converts key events into the appropriate escape sequences that
/// would be sent to a terminal application.
pub fn key_to_bytes(key: KeyEvent) -> Vec<u8> {
  // Handle modifiers
  let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
  let alt = key.modifiers.contains(KeyModifiers::ALT);
  let shift = key.modifiers.contains(KeyModifiers::SHIFT);

  match key.code {
    KeyCode::Char(c) => {
      if ctrl {
        // Control characters
        if c >= 'a' && c <= 'z' {
          vec![(c as u8) - b'a' + 1]
        } else if c >= 'A' && c <= 'Z' {
          vec![(c as u8) - b'A' + 1]
        } else if c == '@' {
          vec![0]
        } else if c == '[' {
          vec![27]
        } else if c == '\\' {
          vec![28]
        } else if c == ']' {
          vec![29]
        } else if c == '^' {
          vec![30]
        } else if c == '_' {
          vec![31]
        } else {
          vec![c as u8]
        }
      } else if alt {
        // Alt + character: ESC followed by character
        vec![0x1b, c as u8]
      } else {
        // Normal character
        vec![c as u8]
      }
    }

    KeyCode::Enter => vec![b'\r'],
    KeyCode::Backspace => vec![0x7f],
    KeyCode::Tab => {
      if shift {
        vec![0x1b, b'[', b'Z'] // Shift+Tab
      } else {
        vec![b'\t']
      }
    }
    KeyCode::Esc => vec![0x1b],

    // Arrow keys
    KeyCode::Up => {
      if ctrl {
        vec![0x1b, b'[', b'1', b';', b'5', b'A']
      } else if alt {
        vec![0x1b, b'[', b'1', b';', b'3', b'A']
      } else if shift {
        vec![0x1b, b'[', b'1', b';', b'2', b'A']
      } else {
        vec![0x1b, b'[', b'A']
      }
    }
    KeyCode::Down => {
      if ctrl {
        vec![0x1b, b'[', b'1', b';', b'5', b'B']
      } else if alt {
        vec![0x1b, b'[', b'1', b';', b'3', b'B']
      } else if shift {
        vec![0x1b, b'[', b'1', b';', b'2', b'B']
      } else {
        vec![0x1b, b'[', b'B']
      }
    }
    KeyCode::Right => {
      if ctrl {
        vec![0x1b, b'[', b'1', b';', b'5', b'C']
      } else if alt {
        vec![0x1b, b'[', b'1', b';', b'3', b'C']
      } else if shift {
        vec![0x1b, b'[', b'1', b';', b'2', b'C']
      } else {
        vec![0x1b, b'[', b'C']
      }
    }
    KeyCode::Left => {
      if ctrl {
        vec![0x1b, b'[', b'1', b';', b'5', b'D']
      } else if alt {
        vec![0x1b, b'[', b'1', b';', b'3', b'D']
      } else if shift {
        vec![0x1b, b'[', b'1', b';', b'2', b'D']
      } else {
        vec![0x1b, b'[', b'D']
      }
    }

    // Function keys
    KeyCode::F(n) => match n {
      1 => vec![0x1b, b'O', b'P'],
      2 => vec![0x1b, b'O', b'Q'],
      3 => vec![0x1b, b'O', b'R'],
      4 => vec![0x1b, b'O', b'S'],
      5 => vec![0x1b, b'[', b'1', b'5', b'~'],
      6 => vec![0x1b, b'[', b'1', b'7', b'~'],
      7 => vec![0x1b, b'[', b'1', b'8', b'~'],
      8 => vec![0x1b, b'[', b'1', b'9', b'~'],
      9 => vec![0x1b, b'[', b'2', b'0', b'~'],
      10 => vec![0x1b, b'[', b'2', b'1', b'~'],
      11 => vec![0x1b, b'[', b'2', b'3', b'~'],
      12 => vec![0x1b, b'[', b'2', b'4', b'~'],
      _ => vec![],
    },

    // Editing keys
    KeyCode::Insert => vec![0x1b, b'[', b'2', b'~'],
    KeyCode::Delete => vec![0x1b, b'[', b'3', b'~'],
    KeyCode::Home => vec![0x1b, b'[', b'H'],
    KeyCode::End => vec![0x1b, b'[', b'F'],
    KeyCode::PageUp => vec![0x1b, b'[', b'5', b'~'],
    KeyCode::PageDown => vec![0x1b, b'[', b'6', b'~'],

    _ => vec![],
  }
}

/// Modal input handler result
pub enum ModalInputResult {
  /// Input was consumed by modal
  Consumed,

  /// Input was not consumed, should be passed through
  PassThrough,

  /// Modal requests to be closed
  CloseModal,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_simple_characters() {
    let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty());
    assert_eq!(key_to_bytes(key), b"a");

    let key = KeyEvent::new(KeyCode::Char('Z'), KeyModifiers::empty());
    assert_eq!(key_to_bytes(key), b"Z");
  }

  #[test]
  fn test_control_characters() {
    // Ctrl+C
    let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(key_to_bytes(key), vec![3]);

    // Ctrl+A
    let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
    assert_eq!(key_to_bytes(key), vec![1]);
  }

  #[test]
  fn test_special_keys() {
    let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    assert_eq!(key_to_bytes(key), b"\r");

    let key = KeyEvent::new(KeyCode::Tab, KeyModifiers::empty());
    assert_eq!(key_to_bytes(key), b"\t");

    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::empty());
    assert_eq!(key_to_bytes(key), vec![0x1b]);
  }

  #[test]
  fn test_arrow_keys() {
    let key = KeyEvent::new(KeyCode::Up, KeyModifiers::empty());
    assert_eq!(key_to_bytes(key), b"\x1b[A");

    let key = KeyEvent::new(KeyCode::Down, KeyModifiers::empty());
    assert_eq!(key_to_bytes(key), b"\x1b[B");

    let key = KeyEvent::new(KeyCode::Right, KeyModifiers::empty());
    assert_eq!(key_to_bytes(key), b"\x1b[C");

    let key = KeyEvent::new(KeyCode::Left, KeyModifiers::empty());
    assert_eq!(key_to_bytes(key), b"\x1b[D");
  }

  #[test]
  fn test_function_keys() {
    let key = KeyEvent::new(KeyCode::F(1), KeyModifiers::empty());
    assert_eq!(key_to_bytes(key), b"\x1bOP");

    let key = KeyEvent::new(KeyCode::F(5), KeyModifiers::empty());
    assert_eq!(key_to_bytes(key), b"\x1b[15~");
  }

  #[test]
  fn test_alt_characters() {
    let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT);
    assert_eq!(key_to_bytes(key), vec![0x1b, b'x']);
  }

  #[test]
  fn test_modified_arrows() {
    // Ctrl+Up
    let key = KeyEvent::new(KeyCode::Up, KeyModifiers::CONTROL);
    assert_eq!(key_to_bytes(key), b"\x1b[1;5A");

    // Shift+Right
    let key = KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT);
    assert_eq!(key_to_bytes(key), b"\x1b[1;2C");
  }
}
