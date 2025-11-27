//! Rendering components for the hybrid terminal
//!
//! This module contains components for rendering via ratatui:
//! - `ratatui_renderer`: Main renderer for terminal content and modals
//! - `modal`: Modal state and content types

pub mod modal;
pub mod ratatui_renderer;

pub use modal::{ModalContent, ModalState, ModalStyle};
pub use ratatui_renderer::RatatuiRenderer;
