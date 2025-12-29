// TERMIN.AI: LLM-related types and utilities

pub mod client;
pub mod forwarded_props;
pub mod integration_example;
pub mod subscriber;
pub mod terminal_context;
pub mod tool_coordinator;
pub mod tool_executor;

pub use client::{AgUiClient, ChatStreamResponse};
pub use forwarded_props::TerminAIForwardedProps;
pub use terminal_context::TerminalContext;
pub use tool_coordinator::{
  ToolCoordinator, ToolExecutionEvent, run_tool_execution_loop,
};
pub use tool_executor::{
  CommandSuggestion, ToolExecutionContext, ToolExecutionRequest, ToolExecutor,
  ToolResult,
};

// Re-export official AG-UI types
pub use ag_ui_core::event::Event as AgUiEvent;
pub use ag_ui_core::types::context::Context;
pub use ag_ui_core::types::message::Message;
pub use ag_ui_core::types::tool::Tool;

// Re-export Provider from llm_old for backward compatibility
pub use crate::llm_old::Provider;
