//! Main event loop for the hybrid terminal
//!
//! This module provides the HybridTerminal struct which orchestrates all
//! components and runs the main event loop.

use crossterm::event::{self, Event as CrosstermEvent, KeyCode};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock, mpsc};

use crate::vt100::TermReplySender;

use super::{
  input::key_to_bytes,
  mode::{Mode, ModeManager},
  rendering::{ModalState, RatatuiRenderer},
  routing::{OutputBuffer, OutputRouter},
  terminal::{HostTerminalController, ShadowTerminal},
};

/// Application events
#[derive(Debug, Clone)]
pub enum AppEvent {
  /// Toggle modal visibility
  ToggleModal,

  /// Close modal
  CloseModal,

  /// Quit the application
  Quit,

  /// Terminal resize event
  Resize(u16, u16),
}

/// Error type for the hybrid terminal
#[derive(Debug)]
pub enum HybridTerminalError {
  /// I/O error
  Io(std::io::Error),

  /// Router error
  Router(super::routing::RouterError),

  /// Channel error
  Channel(String),
}

impl From<std::io::Error> for HybridTerminalError {
  fn from(err: std::io::Error) -> Self {
    HybridTerminalError::Io(err)
  }
}

impl From<super::routing::RouterError> for HybridTerminalError {
  fn from(err: super::routing::RouterError) -> Self {
    HybridTerminalError::Router(err)
  }
}

impl std::fmt::Display for HybridTerminalError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      HybridTerminalError::Io(e) => write!(f, "I/O error: {}", e),
      HybridTerminalError::Router(e) => write!(f, "Router error: {}", e),
      HybridTerminalError::Channel(e) => write!(f, "Channel error: {}", e),
    }
  }
}

impl std::error::Error for HybridTerminalError {}

/// Main hybrid terminal orchestrator
pub struct HybridTerminal<Reply: TermReplySender + Clone> {
  /// Mode manager (shared)
  mode_manager: Arc<RwLock<ModeManager>>,

  /// Shadow terminal (shared)
  shadow_terminal: Arc<RwLock<ShadowTerminal<Reply>>>,

  /// Host terminal controller (shared)
  host_controller: Arc<Mutex<HostTerminalController>>,

  /// Output router (shared)
  output_router: OutputRouter<Reply>,

  /// Ratatui renderer
  renderer: Option<RatatuiRenderer>,

  /// Output buffer (shared)
  output_buffer: Arc<Mutex<OutputBuffer>>,

  /// Current modal state
  modal_state: Option<ModalState>,

  /// Channel for PTY output
  pty_output_rx: mpsc::UnboundedReceiver<Vec<u8>>,

  /// Channel for sending input to PTY
  pty_input_tx: mpsc::UnboundedSender<Vec<u8>>,

  /// Channel for application events
  app_event_rx: mpsc::UnboundedReceiver<AppEvent>,
}

impl<Reply: TermReplySender + Clone + Send + 'static> HybridTerminal<Reply> {
  /// Create a new hybrid terminal
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    cols: u16,
    rows: u16,
    scrollback_lines: usize,
    reply_sender: Reply,
    pty_output_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    pty_input_tx: mpsc::UnboundedSender<Vec<u8>>,
    app_event_rx: mpsc::UnboundedReceiver<AppEvent>,
  ) -> Result<Self, HybridTerminalError> {
    // Create shared components
    let mode_manager = Arc::new(RwLock::new(ModeManager::new()));
    let shadow_terminal = Arc::new(RwLock::new(ShadowTerminal::new(
      cols,
      rows,
      scrollback_lines,
      reply_sender,
    )));
    let host_controller = Arc::new(Mutex::new(HostTerminalController::new()));
    let output_buffer = Arc::new(Mutex::new(OutputBuffer::default()));

    let output_router = OutputRouter::new(
      Arc::clone(&mode_manager),
      Arc::clone(&shadow_terminal),
      Arc::clone(&host_controller),
      Arc::clone(&output_buffer),
    );

    Ok(Self {
      mode_manager,
      shadow_terminal,
      host_controller,
      output_router,
      renderer: None,
      output_buffer,
      modal_state: None,
      pty_output_rx,
      pty_input_tx,
      app_event_rx,
    })
  }

  /// Run the main event loop
  pub async fn run(&mut self) -> Result<(), HybridTerminalError> {
    loop {
      tokio::select! {
          // Handle PTY output
          Some(data) = self.pty_output_rx.recv() => {
              self.handle_pty_output(&data).await?;
          }

          // Handle application events
          Some(event) = self.app_event_rx.recv() => {
              if self.handle_app_event(event).await? {
                  break; // Quit requested
              }
          }

          // Render tick (60 FPS) - also handle keyboard input here
          _ = tokio::time::sleep(Duration::from_millis(16)) => {
              // Check for keyboard events (non-blocking)
              if let Ok(Ok(true)) = tokio::time::timeout(
                  Duration::from_millis(1),
                  Self::has_keyboard_event()
              ).await {
                  if let Ok(evt) = event::read() {
                      if let CrosstermEvent::Key(key) = evt {
                          self.handle_key_input(key).await?;
                      }
                  }
              }

              // Render if needed
              self.maybe_render().await?;
          }
      }
    }

    // Clean up
    self.cleanup().await?;

    Ok(())
  }

  /// Check if keyboard event is available (async wrapper for poll)
  async fn has_keyboard_event() -> std::io::Result<bool> {
    tokio::task::spawn_blocking(|| event::poll(Duration::from_millis(0)))
      .await
      .unwrap_or(Ok(false))
  }

  /// Handle PTY output
  async fn handle_pty_output(
    &mut self,
    data: &[u8],
  ) -> Result<(), HybridTerminalError> {
    self.output_router.route_output(data).await?;
    Ok(())
  }

  /// Handle application event
  async fn handle_app_event(
    &mut self,
    event: AppEvent,
  ) -> Result<bool, HybridTerminalError> {
    match event {
      AppEvent::ToggleModal => {
        self.toggle_modal().await?;
      }
      AppEvent::CloseModal => {
        self.close_modal().await?;
      }
      AppEvent::Resize(cols, rows) => {
        self.handle_resize(cols, rows).await?;
      }
      AppEvent::Quit => {
        return Ok(true); // Signal to quit
      }
    }
    Ok(false)
  }

  /// Toggle modal visibility
  async fn toggle_modal(&mut self) -> Result<(), HybridTerminalError> {
    let is_visible = self.mode_manager.read().await.is_modal_visible();
    if is_visible {
      self.close_modal().await
    } else {
      self.show_modal().await
    }
  }

  /// Show modal
  async fn show_modal(&mut self) -> Result<(), HybridTerminalError> {
    let transition = {
      let mut mode_mgr = self.mode_manager.write().await;
      mode_mgr.set_modal_visible(true)
    };

    // Sync host buffer
    self
      .output_router
      .synchronize_host_buffer(&transition)
      .await?;

    // Create renderer if needed
    if self.renderer.is_none() {
      let stdout = std::io::stdout();
      self.renderer = Some(RatatuiRenderer::new(stdout)?);
    }

    // Create default modal state if none exists
    if self.modal_state.is_none() {
      self.modal_state = Some(ModalState::text(
        "AI Assistant",
        "Press ESC to close\nPress Ctrl+Space to toggle modal",
      ));
    }

    // Force render
    self.force_render().await?;

    Ok(())
  }

  /// Close modal
  async fn close_modal(&mut self) -> Result<(), HybridTerminalError> {
    let transition = {
      let mut mode_mgr = self.mode_manager.write().await;
      mode_mgr.set_modal_visible(false)
    };

    // Sync host buffer (includes replay if needed)
    self
      .output_router
      .synchronize_host_buffer(&transition)
      .await?;

    self.modal_state = None;

    Ok(())
  }

  /// Handle terminal resize
  async fn handle_resize(
    &mut self,
    cols: u16,
    rows: u16,
  ) -> Result<(), HybridTerminalError> {
    // Resize shadow terminal
    {
      let mut shadow = self.shadow_terminal.write().await;
      shadow.resize(cols, rows);
    }

    // Force render if in ratatui mode
    let mode = self.mode_manager.read().await.current_mode();
    if mode != Mode::Passthrough {
      self.force_render().await?;
    }

    Ok(())
  }

  /// Handle keyboard input
  async fn handle_key_input(
    &mut self,
    key: crossterm::event::KeyEvent,
  ) -> Result<(), HybridTerminalError> {
    let mode = self.mode_manager.read().await.current_mode();

    // Check for global shortcuts first
    if key.code == KeyCode::Char(' ')
      && key
        .modifiers
        .contains(crossterm::event::KeyModifiers::CONTROL)
    {
      self.toggle_modal().await?;
      return Ok(());
    }

    match mode {
      Mode::Passthrough | Mode::GuestAltBuffer => {
        // Forward to PTY
        let bytes = key_to_bytes(key);
        self
          .pty_input_tx
          .send(bytes)
          .map_err(|e| HybridTerminalError::Channel(e.to_string()))?;
      }

      Mode::ModalWithBuffering | Mode::ModalGuestAlt => {
        // Modal handles input
        let consumed = self.handle_modal_input(key).await?;
        if !consumed {
          // Not consumed by modal, forward to PTY
          let bytes = key_to_bytes(key);
          self
            .pty_input_tx
            .send(bytes)
            .map_err(|e| HybridTerminalError::Channel(e.to_string()))?;
        }
      }
    }

    Ok(())
  }

  /// Handle modal input
  async fn handle_modal_input(
    &mut self,
    key: crossterm::event::KeyEvent,
  ) -> Result<bool, HybridTerminalError> {
    match key.code {
      KeyCode::Esc => {
        self.close_modal().await?;
        Ok(true)
      }
      KeyCode::Up => {
        if let Some(ref mut modal) = self.modal_state {
          modal.select_previous();
        }
        Ok(true)
      }
      KeyCode::Down => {
        if let Some(ref mut modal) = self.modal_state {
          modal.select_next();
        }
        Ok(true)
      }
      _ => Ok(false), // Not consumed
    }
  }

  /// Render if needed
  async fn maybe_render(&mut self) -> Result<(), HybridTerminalError> {
    let mode = self.mode_manager.read().await.current_mode();

    if mode == Mode::Passthrough {
      // No ratatui rendering needed
      return Ok(());
    }

    self.force_render().await
  }

  /// Force a render
  async fn force_render(&mut self) -> Result<(), HybridTerminalError> {
    if let Some(ref mut renderer) = self.renderer {
      let content = {
        let shadow = self.shadow_terminal.read().await;
        shadow.visible_content()
      };

      renderer.render_frame(&content, self.modal_state.as_mut())?;
    }

    Ok(())
  }

  /// Clean up before exit
  async fn cleanup(&mut self) -> Result<(), HybridTerminalError> {
    // Make sure we leave alt buffer if we're in it
    let mut host = self.host_controller.lock().await;
    host.leave_alt_buffer()?;
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Clone, Debug)]
  struct NoOpReplySender;

  impl TermReplySender for NoOpReplySender {
    fn reply(&self, _s: compact_str::CompactString) {}
  }

  #[tokio::test]
  async fn test_create_hybrid_terminal() {
    let (_pty_output_tx, pty_output_rx) = mpsc::unbounded_channel();
    let (pty_input_tx, _pty_input_rx) = mpsc::unbounded_channel();
    let (_app_event_tx, app_event_rx) = mpsc::unbounded_channel();

    let result = HybridTerminal::new(
      80,
      24,
      1000,
      NoOpReplySender,
      pty_output_rx,
      pty_input_tx,
      app_event_rx,
    );

    assert!(result.is_ok());
  }
}
