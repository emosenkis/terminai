// TERMIN.AI: LLM client module for AI assistance
// NOTE: This is the OLD implementation using rig-core
// Will be replaced with AG-UI based implementation

pub mod ag_ui_client; // NEW: High-level AG-UI client
pub mod ag_ui_transport; // NEW: Transport layer for AG-UI
pub mod client; // OLD: Will be replaced
pub mod prompts; // OLD: Moved to Python
pub mod providers; // OLD: Will be replaced
pub mod tools; // OLD: Will be replaced with AG-UI tools

pub use ag_ui_client::{
  AgUiClient, Message, Role, StreamEvent,
  TerminalContext as AgUiTerminalContext,
}; // NEW
pub use ag_ui_transport::AgUiTransport; // NEW
pub use client::{ChatMessage, LLMClient, TerminalContext}; // OLD
pub use providers::Provider; // OLD
