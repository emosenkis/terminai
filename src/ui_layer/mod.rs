//! UI Layer module for Termin.AI
//!
//! This module provides the `TerminalWidget` for rendering VT100 terminal content
//! to a tui buffer. The widget is used by the main application to display the
//! terminal emulator output.
//!
//! # Usage
//!
//! ```ignore
//! use termin::ui_layer::TerminalWidget;
//!
//! // Render terminal content to a buffer
//! let widget = TerminalWidget::new(vt_screen);
//! widget.render(area, buf);
//!
//! // With row offset (for shifting viewport when overlay is visible)
//! let widget = TerminalWidget::with_offset(vt_screen, 5);
//! widget.render(area, buf);
//! ```

pub mod terminal_layer;

pub use terminal_layer::TerminalWidget;
