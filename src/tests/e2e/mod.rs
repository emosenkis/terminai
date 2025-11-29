// E2E test harness for Termin.AI using ratatui TestBackend
//
// This module provides a comprehensive test harness for end-to-end testing
// of the Termin.AI application using ratatui's TestBackend.

pub mod test_ai;
pub mod test_ui;
pub mod test_vt100;

use anyhow::Result;
use std::time::Duration;
use tui::Terminal;
use tui::backend::{Backend, TestBackend};
use tui::buffer::Buffer;

/// Simple test configuration
#[derive(Debug, Clone)]
pub struct TestAppConfig {
  /// Terminal size (width, height)
  pub terminal_size: (u16, u16),
}

impl Default for TestAppConfig {
  fn default() -> Self {
    Self {
      terminal_size: (80, 24),
    }
  }
}

impl TestAppConfig {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn with_terminal_size(mut self, width: u16, height: u16) -> Self {
    self.terminal_size = (width, height);
    self
  }
}

/// Events that can be sent to the test app
#[derive(Debug, Clone)]
pub enum TestEvent {
  /// Key press
  Key(crossterm::event::KeyEvent),
  /// Resize terminal
  Resize(u16, u16),
  /// Wait for a duration
  Wait(Duration),
}

/// Result from a test step
#[derive(Debug)]
pub struct TestStepResult {
  /// The terminal buffer after this step
  pub buffer: Buffer,
  /// Any output from the shell
  pub shell_output: Vec<String>,
}

/// Test harness for e2e testing
pub struct TestHarness {
  /// Test backend
  backend: TestBackend,
  /// Terminal
  terminal: Terminal<TestBackend>,
  /// Test configuration
  config: TestAppConfig,
  /// Event queue
  events: Vec<TestEvent>,
}

impl TestHarness {
  /// Create a new test harness with default configuration
  pub fn new() -> Self {
    Self::with_config(TestAppConfig::default())
  }

  /// Create a new test harness with custom configuration
  pub fn with_config(config: TestAppConfig) -> Self {
    let (width, height) = config.terminal_size;
    let backend = TestBackend::new(width, height);
    let terminal = Terminal::new(backend).expect("Failed to create terminal");

    Self {
      backend: TestBackend::new(width, height),
      terminal,
      config,
      events: Vec::new(),
    }
  }

  /// Get the terminal size
  pub fn size(&self) -> (u16, u16) {
    self.config.terminal_size
  }

  /// Add a key event to the event queue
  pub fn key(&mut self, key: crossterm::event::KeyEvent) -> &mut Self {
    self.events.push(TestEvent::Key(key));
    self
  }

  /// Add a key press event (simplified)
  pub fn press_key(&mut self, code: crossterm::event::KeyCode) -> &mut Self {
    use crossterm::event::{KeyEvent, KeyEventKind, KeyModifiers};
    self.key(KeyEvent {
      code,
      modifiers: KeyModifiers::NONE,
      kind: KeyEventKind::Press,
      state: crossterm::event::KeyEventState::empty(),
    })
  }

  /// Add a key press with modifiers
  pub fn press_key_with_modifiers(
    &mut self,
    code: crossterm::event::KeyCode,
    modifiers: crossterm::event::KeyModifiers,
  ) -> &mut Self {
    use crossterm::event::{KeyEvent, KeyEventKind};
    self.key(KeyEvent {
      code,
      modifiers,
      kind: KeyEventKind::Press,
      state: crossterm::event::KeyEventState::empty(),
    })
  }

  /// Add a resize event
  pub fn resize(&mut self, width: u16, height: u16) -> &mut Self {
    self.events.push(TestEvent::Resize(width, height));
    self
  }

  /// Add a wait event
  pub fn wait(&mut self, duration: Duration) -> &mut Self {
    self.events.push(TestEvent::Wait(duration));
    self
  }

  /// Type a string (sends each character as a key event)
  pub fn type_string(&mut self, text: &str) -> &mut Self {
    for ch in text.chars() {
      self.press_key(crossterm::event::KeyCode::Char(ch));
    }
    self
  }

  /// Get a reference to the backend
  pub fn backend(&self) -> &TestBackend {
    &self.backend
  }

  /// Get a mutable reference to the backend
  pub fn backend_mut(&mut self) -> &mut TestBackend {
    &mut self.backend
  }

  /// Get the current buffer as a string
  pub fn buffer_as_string(&self) -> String {
    // Extract text content from the buffer
    let buffer = self.backend.buffer();
    let mut content = String::new();
    for y in 0..buffer.area().height {
      for x in 0..buffer.area().width {
        let pos = tui::layout::Position { x, y };
        if let Some(cell) = buffer.cell(pos) {
          content.push_str(cell.symbol());
        }
      }
      content.push('\n');
    }
    content
  }

  /// Get the buffer
  pub fn buffer(&self) -> &Buffer {
    self.backend.buffer()
  }

  /// Assert that the buffer contains a specific string
  pub fn assert_buffer_contains(&self, text: &str) {
    let buffer_str = self.buffer_as_string();
    assert!(
      buffer_str.contains(text),
      "Buffer does not contain '{}'\nBuffer contents:\n{}",
      text,
      buffer_str
    );
  }

  /// Assert that the buffer matches specific lines
  pub fn assert_buffer_lines<'line, Lines>(&self, expected: Lines)
  where
    Lines: IntoIterator,
    Lines::Item: Into<tui::text::Line<'line>>,
  {
    self.backend.assert_buffer_lines(expected);
  }

  /// Clear the terminal
  pub fn clear(&mut self) -> Result<()> {
    self.backend.clear()?;
    Ok(())
  }

  /// Render a widget to the terminal
  pub fn render<W>(&mut self, widget: W) -> Result<()>
  where
    W: tui::widgets::Widget,
  {
    self.terminal.draw(|f| {
      let area = f.area();
      f.render_widget(widget, area);
    })?;
    // Update our backend reference
    self.backend = self.terminal.backend().clone();
    Ok(())
  }

  /// Get the test configuration
  pub fn config(&self) -> &TestAppConfig {
    &self.config
  }
}

impl Default for TestHarness {
  fn default() -> Self {
    Self::new()
  }
}

/// Helper to create a simple key press event
pub fn key_press(
  code: crossterm::event::KeyCode,
) -> crossterm::event::KeyEvent {
  use crossterm::event::{KeyEvent, KeyEventKind, KeyModifiers};
  KeyEvent {
    code,
    modifiers: KeyModifiers::NONE,
    kind: KeyEventKind::Press,
    state: crossterm::event::KeyEventState::empty(),
  }
}

/// Helper to create a key press with modifiers
pub fn key_press_with_modifiers(
  code: crossterm::event::KeyCode,
  modifiers: crossterm::event::KeyModifiers,
) -> crossterm::event::KeyEvent {
  use crossterm::event::{KeyEvent, KeyEventKind};
  KeyEvent {
    code,
    modifiers,
    kind: KeyEventKind::Press,
    state: crossterm::event::KeyEventState::empty(),
  }
}

/// Helper to create ctrl+key event
pub fn ctrl_key(ch: char) -> crossterm::event::KeyEvent {
  use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
  KeyEvent {
    code: KeyCode::Char(ch),
    modifiers: KeyModifiers::CONTROL,
    kind: KeyEventKind::Press,
    state: crossterm::event::KeyEventState::empty(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_harness_creation() {
    let harness = TestHarness::new();
    assert_eq!(harness.size(), (80, 24));
  }

  #[test]
  fn test_harness_custom_size() {
    let config = TestAppConfig::new().with_terminal_size(120, 40);
    let harness = TestHarness::with_config(config);
    assert_eq!(harness.size(), (120, 40));
  }

  #[test]
  fn test_event_building() {
    let mut harness = TestHarness::new();
    harness
      .type_string("hello")
      .press_key(crossterm::event::KeyCode::Enter)
      .wait(Duration::from_millis(100));

    assert_eq!(harness.events.len(), 7); // 5 chars + enter + wait
  }

  #[test]
  fn test_buffer_rendering() {
    use tui::widgets::{Block, Borders, Paragraph};

    let mut harness = TestHarness::new();
    let widget = Paragraph::new("Test Content")
      .block(Block::default().borders(Borders::ALL));

    harness.render(widget).unwrap();
    harness.assert_buffer_contains("Test Content");
  }
}
