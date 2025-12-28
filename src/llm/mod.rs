// TERMIN.AI: LLM-related types and utilities

pub mod client;
pub mod forwarded_props;
pub mod subscriber;
pub mod terminal_context;

pub use client::AgUiClient;
pub use forwarded_props::TerminAIForwardedProps;
pub use terminal_context::TerminalContext;

// Re-export official AG-UI types
pub use ag_ui_core::event::Event as AgUiEvent;
pub use ag_ui_core::types::context::Context;
pub use ag_ui_core::types::message::Message;
pub use ag_ui_core::types::tool::Tool;

// Re-export Provider from llm_old for backward compatibility
pub use crate::llm_old::Provider;
