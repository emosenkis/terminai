//! Hybrid Terminal System
//!
//! This module implements a hybrid terminal system that can switch between
//! passthrough mode (direct output to host) and ratatui rendering mode
//! (for modal overlays and guest alternate buffer).
//!
//! # Architecture
//!
//! The system operates in four distinct modes based on two boolean states:
//! - Whether the modal is visible
//! - Whether the guest terminal is in alternate buffer
//!
//! ## Modes
//!
//! - **Passthrough**: Direct output to host main buffer, shadow tracks state
//! - **GuestAltBuffer**: Full ratatui rendering (guest in alt buffer, no modal)
//! - **ModalWithBuffering**: Ratatui + output buffering for later replay
//! - **ModalGuestAlt**: Full ratatui rendering (modal + guest in alt buffer)
//!
//! ## Components
//!
//! - `mode`: Mode management and state machine
//! - `terminal`: Shadow terminal, host controller, content representation
//! - `routing`: Output routing and buffering
//! - `rendering`: Ratatui rendering and modal UI
//! - `event_loop`: Main event loop orchestration
//! - `input`: Keyboard input handling
//!
//! # Usage
//!
//! ```no_run
//! use termin::hybrid::HybridTerminal;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create channels for communication
//! let (pty_output_tx, pty_output_rx) = tokio::sync::mpsc::unbounded_channel();
//! let (pty_input_tx, pty_input_rx) = tokio::sync::mpsc::unbounded_channel();
//! let (app_event_tx, app_event_rx) = tokio::sync::mpsc::unbounded_channel();
//!
//! // Create hybrid terminal
//! let mut terminal = HybridTerminal::new(
//!     80, 24,              // cols, rows
//!     1000,                // scrollback lines
//!     reply_sender,        // vt100 reply sender
//!     pty_output_rx,       // receive PTY output
//!     pty_input_tx,        // send input to PTY
//!     app_event_rx,        // receive app events
//! )?;
//!
//! // Run the event loop
//! terminal.run().await?;
//! # Ok(())
//! # }
//! ```

pub mod event_loop;
pub mod input;
pub mod mode;
pub mod rendering;
pub mod routing;
pub mod terminal;

// Re-export main types
pub use event_loop::{AppEvent, HybridTerminal, HybridTerminalError};
pub use input::{ModalInputResult, key_to_bytes};
pub use mode::{Mode, ModeManager, ModeTransition};
pub use rendering::{ModalContent, ModalState, ModalStyle, RatatuiRenderer};
pub use routing::{OutputBuffer, OutputRouter, RouterError};
pub use terminal::{
  HostTerminalController, ShadowTerminal, TerminalContent, TerminalEvent,
};
