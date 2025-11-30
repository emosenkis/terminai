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
//! - `input`: Keyboard input handling
//!
//! # Usage
//!
//! The hybrid terminal system is designed to be used by integrating its
//! components directly into your application. See `src/bin/terminai.rs`
//! for a complete example of how to use these components.

pub mod input;
pub mod mode;
pub mod rendering;
pub mod routing;
pub mod terminal;

// Re-export main types
pub use input::{ModalInputResult, key_to_bytes};
pub use mode::{Mode, ModeManager, ModeTransition};
pub use rendering::{ModalContent, ModalState, ModalStyle, RatatuiRenderer};
pub use routing::{OutputBuffer, OutputRouter, RouterError};
pub use terminal::{
  HostTerminalController, ShadowTerminal, TerminalContent, TerminalEvent,
};
