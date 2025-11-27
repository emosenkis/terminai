//! Output routing components
//!
//! This module contains components for routing terminal output based on
//! the current mode:
//! - `buffer`: Output buffering for modal display with replay
//! - `output_router`: Core routing logic that directs output appropriately

pub mod buffer;
pub mod output_router;

pub use buffer::{OutputBuffer, SmartOutputBuffer};
pub use output_router::{OutputRouter, RouterError};
