// TERMIN.AI: LLM-related types and utilities
//
// This module provides the Deno-based LLM client for communication with
// Claude and other LLMs via the embedded TypeScript agent.

pub mod deno_client;
pub mod providers;
pub mod terminal_context;
pub mod tool_executor;

pub use deno_client::{
  DenoChatStreamResponse, DenoLlmClient, ToolCallNotification,
};
pub use terminal_context::TerminalContext;
pub use tool_executor::{
  CommandSuggestion, ToolCallId, ToolExecutionContext, ToolExecutionRequest,
  ToolExecutor, ToolResult,
};

// Re-export Provider enum
pub use providers::Provider;
