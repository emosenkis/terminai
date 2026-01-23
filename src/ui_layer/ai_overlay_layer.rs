// AI Overlay layer - renders the AI chat interface
// This layer sits on top of the terminal and handles chat interactions

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use rat_cursor::HasScreenCursor;
use rat_event::{HandleEvent, Outcome, Regular};
use std::sync::Arc;
use tokio::sync::Mutex;
use tui::{
  buffer::Buffer,
  layout::Rect,
  style::{Color, Style},
  widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use super::{LayerEventOutcome, UILayer};
use crate::ai_proc::{AIChatProcess, AIChatUI};
use crate::terminai_config::ChatPosition;

/// AI overlay layer - displays the chat interface when activated
pub struct AIOverlayLayer<'a> {
  /// Whether the overlay is currently visible
  visible: bool,
  /// Position of the chat overlay (top or bottom)
  chat_position: ChatPosition,
  /// The AI chat process (handles conversation state)
  ai_process: Option<Arc<Mutex<AIChatProcess>>>,
  /// The AI chat UI widget
  ai_ui: AIChatUI<'a>,
  /// Configuration error message if AI is not configured
  config_error: Option<String>,
  /// Cached overlay area
  overlay_area: Rect,
}

impl<'a> AIOverlayLayer<'a> {
  pub fn new(chat_position: ChatPosition) -> Self {
    Self {
      visible: false,
      chat_position,
      ai_process: None,
      ai_ui: AIChatUI::new(),
      config_error: None,
      overlay_area: Rect::default(),
    }
  }

  /// Set the AI process
  pub fn set_ai_process(&mut self, process: Option<Arc<Mutex<AIChatProcess>>>) {
    self.ai_process = process;
  }

  /// Get a reference to the AI process
  pub fn ai_process(&self) -> &Option<Arc<Mutex<AIChatProcess>>> {
    &self.ai_process
  }

  /// Set the configuration error
  pub fn set_config_error(&mut self, error: Option<String>) {
    self.config_error = error;
  }

  /// Show the overlay
  pub fn show(&mut self) {
    self.visible = true;
  }

  /// Hide the overlay
  pub fn hide(&mut self) {
    self.visible = false;
  }

  /// Toggle the overlay visibility
  pub fn toggle(&mut self) {
    self.visible = !self.visible;
  }

  /// Get the chat position
  pub fn chat_position(&self) -> ChatPosition {
    self.chat_position
  }

  /// Get the AI UI (for focus management)
  pub fn ai_ui(&self) -> &AIChatUI<'a> {
    &self.ai_ui
  }

  /// Get mutable reference to AI UI
  pub fn ai_ui_mut(&mut self) -> &mut AIChatUI<'a> {
    &mut self.ai_ui
  }

  /// Calculate the overlay height based on area
  pub fn calculate_overlay_height(&self, area: Rect) -> u16 {
    (area.height / 2).max(10)
  }

  /// Calculate the overlay area based on position and total area
  pub fn calculate_overlay_area(&self, area: Rect) -> Rect {
    let overlay_height = self.calculate_overlay_height(area);
    let overlay_y = match self.chat_position {
      ChatPosition::Bottom => area.y + area.height - overlay_height,
      ChatPosition::Top => area.y,
    };
    Rect {
      x: area.x,
      y: overlay_y,
      width: area.width,
      height: overlay_height,
    }
  }

  /// Clear any active dialogs (error or approval)
  pub fn clear_dialogs(&mut self) {
    if let Some(ref ai_process_arc) = self.ai_process {
      if let Ok(mut ai_process) = ai_process_arc.try_lock() {
        if ai_process.error_message().is_some() {
          ai_process.clear_error();
        }
        if ai_process.pending_command().is_some() {
          ai_process.reject_command();
        }
      }
    }
  }

  /// Get the input value from the UI
  pub fn get_input_value(&self) -> String {
    self.ai_ui.get_input_value()
  }

  /// Clear the input
  pub fn clear_input(&mut self) {
    self.ai_ui.clear_input();
  }

  /// Check if conversation is focused
  pub fn conversation_focused(&self) -> bool {
    self.ai_ui.conversation_focus().get()
  }

  /// Check if input is focused
  pub fn input_focused(&self) -> bool {
    self.ai_ui.input_focus().get()
  }
}

impl UILayer for AIOverlayLayer<'_> {
  fn is_visible(&self) -> bool {
    self.visible
  }

  fn render(&mut self, area: Rect, buf: &mut Buffer) {
    // Calculate and cache overlay area
    self.overlay_area = self.calculate_overlay_area(area);

    // Clear the overlay area first
    Clear.render(self.overlay_area, buf);

    if let Some(ref ai_process_arc) = self.ai_process {
      // Try to lock without blocking (non-blocking for render)
      if let Ok(ai_process) = ai_process_arc.try_lock() {
        // Render AI chat interface
        self.ai_ui.render(&*ai_process, self.overlay_area, buf);
      } else {
        // Lock is held (AI is processing) - render loading state
        let message = Paragraph::new("Processing... (AI is thinking)").block(
          Block::default()
            .borders(Borders::ALL)
            .title(" AI Assistant ")
            .style(Style::default().fg(Color::Cyan).bg(Color::Black)),
        );
        message.render(self.overlay_area, buf);
      }
    } else {
      // Show "not configured" message with actual error if available
      let error_text = if let Some(ref err) = self.config_error {
        format!(
          "AI Assistant not configured.\n\n\
           Error: {}\n\n\
           Press ESC or Ctrl-Space to close this overlay.",
          err
        )
      } else {
        "AI Assistant not configured.\n\n\
         Configuration error.\n\n\
         Press ESC or Ctrl-Space to close this overlay."
          .to_string()
      };

      let message = Paragraph::new(error_text)
        .block(
          Block::default()
            .borders(Borders::ALL)
            .title(" AI Assistant ")
            .style(Style::default().fg(Color::Yellow)),
        )
        .style(Style::default().fg(Color::White));

      message.render(self.overlay_area, buf);
    }
  }

  fn handle_event(&mut self, event: &Event) -> Result<LayerEventOutcome> {
    if !self.visible {
      return Ok(LayerEventOutcome::Continue);
    }

    // Handle key events
    if let Event::Key(KeyEvent {
      code,
      kind,
      modifiers,
      ..
    }) = event
    {
      // Only process key press/repeat events
      if !matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat) {
        return Ok(LayerEventOutcome::Continue);
      }

      // Check for approval dialog keys
      if let Some(ref ai_process_arc) = self.ai_process {
        if let Ok(mut ai_process) = ai_process_arc.try_lock() {
          // Handle approval dialog
          if ai_process.pending_command().is_some() {
            match code {
              KeyCode::Char('y') | KeyCode::Char('Y') => {
                log::info!("Command approved by user");
                // Note: The caller needs to handle the actual command execution
                // We just approve it here
                ai_process.approve_command();
                return Ok(LayerEventOutcome::Changed);
              }
              KeyCode::Char('n') | KeyCode::Char('N') => {
                log::info!("Command rejected by user");
                ai_process.reject_command();
                return Ok(LayerEventOutcome::Changed);
              }
              _ => {
                // Consume but don't act on other keys when approval dialog is shown
                return Ok(LayerEventOutcome::Unchanged);
              }
            }
          }

          // Handle error dialog
          if ai_process.error_message().is_some() {
            match code {
              KeyCode::Esc => {
                ai_process.clear_error();
                return Ok(LayerEventOutcome::Changed);
              }
              KeyCode::Up => {
                ai_process.error_scroll_up(1);
                return Ok(LayerEventOutcome::Changed);
              }
              KeyCode::Down => {
                ai_process.error_scroll_down(1);
                return Ok(LayerEventOutcome::Changed);
              }
              _ => {
                // Consume but don't act on other keys when error dialog is shown
                return Ok(LayerEventOutcome::Unchanged);
              }
            }
          }
        }
      }

      // Route to conversation if focused
      if self.ai_ui.conversation_focus().get() {
        let outcome =
          HandleEvent::handle(self.ai_ui.conversation_state(), event, Regular);
        return Ok(outcome.into());
      }

      // Route to input if focused
      if self.ai_ui.input_focus().get() {
        // Enter key to send message is handled by the main app
        // (since it needs to spawn async tasks)
        if matches!(code, KeyCode::Enter) && modifiers.is_empty() {
          // Signal that enter was pressed but let the main app handle it
          return Ok(LayerEventOutcome::Continue);
        }

        // Route other keys to input widget
        self.ai_ui.input_event(event);
        return Ok(LayerEventOutcome::Changed);
      }
    }

    // Handle mouse events
    if let Event::Mouse(_mouse) = event {
      // Route to conversation widget
      let outcome = HandleEvent::handle(
        self.ai_ui.conversation_state(),
        event,
        rat_event::MouseOnly,
      );
      if outcome == Outcome::Changed {
        return Ok(LayerEventOutcome::Changed);
      }
    }

    Ok(LayerEventOutcome::Continue)
  }

  fn screen_cursor(&self) -> Option<(u16, u16)> {
    if !self.visible {
      return None;
    }

    // Show cursor in input area when it has focus
    self.ai_ui.input_state().screen_cursor()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_ai_overlay_visibility() {
    let mut layer = AIOverlayLayer::new(ChatPosition::Bottom);
    assert!(!layer.is_visible());

    layer.show();
    assert!(layer.is_visible());

    layer.hide();
    assert!(!layer.is_visible());

    layer.toggle();
    assert!(layer.is_visible());

    layer.toggle();
    assert!(!layer.is_visible());
  }

  #[test]
  fn test_ai_overlay_calculate_area_bottom() {
    let layer = AIOverlayLayer::new(ChatPosition::Bottom);
    let area = Rect::new(0, 0, 80, 24);

    let overlay_area = layer.calculate_overlay_area(area);

    // Overlay should be at bottom
    assert_eq!(overlay_area.x, 0);
    assert_eq!(overlay_area.width, 80);
    // Height is max(24/2, 10) = 12
    assert_eq!(overlay_area.height, 12);
    // Y should be at bottom
    assert_eq!(overlay_area.y, 24 - 12);
  }

  #[test]
  fn test_ai_overlay_calculate_area_top() {
    let layer = AIOverlayLayer::new(ChatPosition::Top);
    let area = Rect::new(0, 0, 80, 24);

    let overlay_area = layer.calculate_overlay_area(area);

    // Overlay should be at top
    assert_eq!(overlay_area.x, 0);
    assert_eq!(overlay_area.y, 0);
    assert_eq!(overlay_area.width, 80);
    assert_eq!(overlay_area.height, 12);
  }

  #[test]
  fn test_ai_overlay_minimum_height() {
    let layer = AIOverlayLayer::new(ChatPosition::Bottom);
    // Small terminal
    let area = Rect::new(0, 0, 80, 15);

    let overlay_height = layer.calculate_overlay_height(area);

    // Should use minimum of 10
    assert_eq!(overlay_height, 10);
  }

  #[test]
  fn test_ai_overlay_events_when_hidden() {
    let mut layer = AIOverlayLayer::new(ChatPosition::Bottom);
    layer.hide();

    let event =
      Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty()));

    let result = layer.handle_event(&event).unwrap();
    assert_eq!(result, LayerEventOutcome::Continue);
  }

  #[test]
  fn test_ai_overlay_no_cursor_when_hidden() {
    let layer = AIOverlayLayer::new(ChatPosition::Bottom);
    assert_eq!(layer.screen_cursor(), None);
  }

  #[test]
  fn test_ai_overlay_set_process() {
    let mut layer = AIOverlayLayer::new(ChatPosition::Bottom);
    assert!(layer.ai_process().is_none());

    // Note: We can't easily create an AIChatProcess in tests without mocking
    // Just test that set_ai_process doesn't panic with None
    layer.set_ai_process(None);
    assert!(layer.ai_process().is_none());
  }

  #[test]
  fn test_ai_overlay_config_error() {
    let mut layer = AIOverlayLayer::new(ChatPosition::Bottom);
    assert!(layer.config_error.is_none());

    layer.set_config_error(Some("Test error".to_string()));
    assert_eq!(layer.config_error, Some("Test error".to_string()));
  }
}
