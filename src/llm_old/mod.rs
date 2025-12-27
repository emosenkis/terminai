// TERMIN.AI: LLM client module for AI assistance via AG-UI protocol
// Communicates with Python subprocess running Pydantic AI agent

pub mod ag_ui_client; // High-level AG-UI client
pub mod ag_ui_transport; // Transport layer for AG-UI (subprocess + HTTP)
pub mod providers; // Provider enum (anthropic, openai, etc.)

pub use ag_ui_client::{
  AgUiClient, Message, Role, StreamEvent,
  TerminalContext as AgUiTerminalContext,
};
pub use ag_ui_transport::AgUiTransport;
pub use providers::Provider;
