//! Output routing based on mode
//!
//! This module provides the core routing logic that determines where terminal
//! output goes based on the current operational mode.

use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

use crate::vt100::TermReplySender;

use super::buffer::OutputBuffer;
use crate::hybrid::{
  mode::{Mode, ModeManager, ModeTransition},
  terminal::{HostTerminalController, ShadowTerminal, TerminalEvent},
};

/// Error type for output routing
#[derive(Debug)]
pub enum RouterError {
  /// I/O error writing to host
  IoError(std::io::Error),

  /// Mode manager lock poisoned
  LockError,
}

impl From<std::io::Error> for RouterError {
  fn from(err: std::io::Error) -> Self {
    RouterError::IoError(err)
  }
}

impl std::fmt::Display for RouterError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      RouterError::IoError(e) => write!(f, "I/O error: {}", e),
      RouterError::LockError => write!(f, "Lock error"),
    }
  }
}

impl std::error::Error for RouterError {}

/// Routes terminal output to appropriate destinations based on mode
///
/// This is the central component that implements the mode-based routing logic:
/// - Passthrough: Write directly to host
/// - GuestAltBuffer: Shadow only (ratatui renders)
/// - ModalWithBuffering: Buffer + shadow (ratatui renders)
/// - ModalGuestAlt: Shadow only (ratatui renders)
pub struct OutputRouter<Reply: TermReplySender + Clone> {
  /// Mode manager (shared)
  mode_manager: Arc<RwLock<ModeManager>>,

  /// Shadow terminal (shared)
  shadow_terminal: Arc<RwLock<ShadowTerminal<Reply>>>,

  /// Host terminal controller (shared)
  host_controller: Arc<Mutex<HostTerminalController>>,

  /// Output buffer for ModalWithBuffering mode
  output_buffer: Arc<Mutex<OutputBuffer>>,
}

impl<Reply: TermReplySender + Clone> OutputRouter<Reply> {
  /// Create a new output router
  pub fn new(
    mode_manager: Arc<RwLock<ModeManager>>,
    shadow_terminal: Arc<RwLock<ShadowTerminal<Reply>>>,
    host_controller: Arc<Mutex<HostTerminalController>>,
    output_buffer: Arc<Mutex<OutputBuffer>>,
  ) -> Self {
    Self {
      mode_manager,
      shadow_terminal,
      host_controller,
      output_buffer,
    }
  }

  /// Process output from the guest PTY
  ///
  /// This is the main entry point for routing terminal output.
  /// It updates the shadow terminal and routes the output based on mode.
  pub async fn route_output(&self, data: &[u8]) -> Result<(), RouterError> {
    if data.is_empty() {
      return Ok(());
    }

    // Get current mode before processing
    let mode = {
      let mode_mgr = self.mode_manager.read().await;
      mode_mgr.current_mode()
    };

    // Always update shadow terminal (it tracks full state)
    let events = {
      let mut shadow = self.shadow_terminal.write().await;
      shadow.process(data)
    };

    // Handle any detected events (like alt buffer switches)
    for event in events {
      self.handle_terminal_event(event).await?;
    }

    // Route based on mode
    match mode {
      Mode::Passthrough => {
        // Direct passthrough to host
        let mut host = self.host_controller.lock().await;
        host.write_raw(data)?;
      }

      Mode::GuestAltBuffer | Mode::ModalGuestAlt => {
        // Ratatui handles all rendering, nothing to passthrough
        // The render loop will pick up changes from shadow terminal
      }

      Mode::ModalWithBuffering => {
        // Buffer for later replay when modal closes
        let mut buffer = self.output_buffer.lock().await;
        buffer.append(data);
        // Ratatui render loop handles display from shadow terminal
      }
    }

    Ok(())
  }

  /// Handle a terminal event (like alt buffer switch)
  async fn handle_terminal_event(
    &self,
    event: TerminalEvent,
  ) -> Result<(), RouterError> {
    match event {
      TerminalEvent::AltBufferChange(entering_alt) => {
        self.handle_alt_buffer_change(entering_alt).await?;
      }
      TerminalEvent::TitleChange(_title) => {
        // Could be used to update window title
        // For now, we ignore this
      }
      TerminalEvent::Bell => {
        // Could trigger visual/audio feedback
        // For now, we ignore this
      }
    }
    Ok(())
  }

  /// Handle guest alt buffer state change
  async fn handle_alt_buffer_change(
    &self,
    entering_alt: bool,
  ) -> Result<(), RouterError> {
    let transition = {
      let mut mode_mgr = self.mode_manager.write().await;
      mode_mgr.set_guest_alt_buffer(entering_alt)
    };

    // Handle mode transition
    self.synchronize_host_buffer(&transition).await?;

    // Special handling for modal + guest alt buffer transitions
    match (transition.from, transition.to) {
      (Mode::ModalWithBuffering, Mode::ModalGuestAlt) => {
        // Guest entered alt buffer while modal visible
        // Clear the buffer - we won't need to replay it
        let mut buffer = self.output_buffer.lock().await;
        buffer.clear();
      }
      _ => {
        // Other transitions handled by normal sync logic
      }
    }

    Ok(())
  }

  /// Synchronize host buffer state based on mode transition
  pub async fn synchronize_host_buffer(
    &self,
    transition: &ModeTransition,
  ) -> Result<(), RouterError> {
    if !transition.needs_host_buffer_switch {
      return Ok(());
    }

    let mut host = self.host_controller.lock().await;
    let mode_mgr = self.mode_manager.read().await;

    if mode_mgr.requires_host_alt_buffer() {
      // Enter alt buffer
      host.enter_alt_buffer()?;

      // Update tracking
      drop(host);
      drop(mode_mgr);
      let mut mode_mgr = self.mode_manager.write().await;
      mode_mgr.set_host_alt_buffer(true);
    } else {
      // Leaving alt buffer
      if transition.needs_buffer_replay {
        // Get buffered output before leaving
        let buffered_data = {
          let mut buffer = self.output_buffer.lock().await;
          buffer.take()
        };

        // Leave alt buffer first
        host.leave_alt_buffer()?;

        // Update tracking
        drop(host);
        drop(mode_mgr);
        let mut mode_mgr = self.mode_manager.write().await;
        mode_mgr.set_host_alt_buffer(false);
        drop(mode_mgr);

        // Then replay buffered output to main buffer
        let mut host = self.host_controller.lock().await;
        host.write_raw(&buffered_data)?;
      } else {
        host.leave_alt_buffer()?;

        // Update tracking
        drop(host);
        drop(mode_mgr);
        let mut mode_mgr = self.mode_manager.write().await;
        mode_mgr.set_host_alt_buffer(false);
      }
    }

    Ok(())
  }
}

impl<Reply: TermReplySender + Clone> Clone for OutputRouter<Reply> {
  fn clone(&self) -> Self {
    Self {
      mode_manager: Arc::clone(&self.mode_manager),
      shadow_terminal: Arc::clone(&self.shadow_terminal),
      host_controller: Arc::clone(&self.host_controller),
      output_buffer: Arc::clone(&self.output_buffer),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Clone, Debug)]
  struct NoOpReplySender;

  impl TermReplySender for NoOpReplySender {
    fn send(&self, _data: Vec<u8>) {}
  }

  fn create_test_router() -> OutputRouter<NoOpReplySender> {
    let mode_manager = Arc::new(RwLock::new(ModeManager::new()));
    let shadow = Arc::new(RwLock::new(ShadowTerminal::new(
      80,
      24,
      1000,
      NoOpReplySender,
    )));
    let host = Arc::new(Mutex::new(HostTerminalController::new()));
    let buffer = Arc::new(Mutex::new(OutputBuffer::new(1024 * 1024)));

    OutputRouter::new(mode_manager, shadow, host, buffer)
  }

  #[tokio::test]
  async fn test_passthrough_mode() {
    let router = create_test_router();

    // In passthrough mode, data should go to host
    let result = router.route_output(b"Hello").await;
    assert!(result.is_ok());

    // Verify shadow terminal got updated
    let shadow = router.shadow_terminal.read().await;
    let content = shadow.visible_content();
    assert_eq!(content.cells[0][0].symbol(), "H");
  }

  #[tokio::test]
  async fn test_guest_alt_buffer_transition() {
    let router = create_test_router();

    // Send alt buffer enter sequence
    let result = router.route_output(b"\x1b[?1049h").await;
    assert!(result.is_ok());

    // Mode should have changed
    let mode = router.mode_manager.read().await.current_mode();
    assert_eq!(mode, Mode::GuestAltBuffer);
  }

  #[tokio::test]
  async fn test_modal_buffering() {
    let router = create_test_router();

    // Show modal
    {
      let mut mode_mgr = router.mode_manager.write().await;
      let transition = mode_mgr.set_modal_visible(true);
      drop(mode_mgr);

      // Sync host buffer
      router.synchronize_host_buffer(&transition).await.unwrap();
    }

    // Should be in ModalWithBuffering mode
    let mode = router.mode_manager.read().await.current_mode();
    assert_eq!(mode, Mode::ModalWithBuffering);

    // Route some output
    router.route_output(b"Buffered data").await.unwrap();

    // Buffer should contain data
    let buffer = router.output_buffer.lock().await;
    assert!(!buffer.is_empty());
  }

  #[tokio::test]
  async fn test_buffer_replay() {
    let router = create_test_router();

    // Show modal and buffer some data
    {
      let mut mode_mgr = router.mode_manager.write().await;
      let transition = mode_mgr.set_modal_visible(true);
      drop(mode_mgr);
      router.synchronize_host_buffer(&transition).await.unwrap();
    }

    router.route_output(b"Buffered").await.unwrap();

    // Hide modal (should trigger replay)
    {
      let mut mode_mgr = router.mode_manager.write().await;
      let transition = mode_mgr.set_modal_visible(false);
      drop(mode_mgr);
      router.synchronize_host_buffer(&transition).await.unwrap();
    }

    // Buffer should be empty after replay
    let buffer = router.output_buffer.lock().await;
    assert!(buffer.is_empty());
  }
}
