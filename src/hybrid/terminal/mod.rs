//! Terminal components for the hybrid terminal system
//!
//! This module contains components for managing terminal state:
//! - `shadow`: Shadow terminal that mirrors guest terminal state
//! - `host_controller`: Controls the host terminal (alternate buffer, etc.)
//! - `content`: Terminal content representation for ratatui rendering

pub mod content;
pub mod host_controller;
pub mod shadow;

pub use content::{TerminalContent, map_cell, map_color};
pub use host_controller::HostTerminalController;
pub use shadow::{ShadowTerminal, TerminalEvent};
