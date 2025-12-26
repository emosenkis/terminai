// TERMIN.AI: LLM client module for AI assistance
//
// This module provides LLM integration using Python's PydanticAI
// library through PyO3 bindings.

pub mod client;
pub mod prompts;
pub mod providers;
pub mod python_bridge;

pub use client::{LLMClient, SuggestedCommand};
pub use providers::Provider;

// Re-export types that were previously in client.rs
use std::path::PathBuf;

/// Terminal context passed to the LLM
#[derive(Debug, Clone)]
pub struct TerminalContext {
  pub history_lines: Vec<String>,
  pub cwd: PathBuf,
  pub last_exit_code: Option<i32>,
}

impl TerminalContext {
  pub fn new(
    history_lines: Vec<String>,
    cwd: PathBuf,
    last_exit_code: Option<i32>,
  ) -> Self {
    Self {
      history_lines,
      cwd,
      last_exit_code,
    }
  }

  pub fn empty(cwd: PathBuf) -> Self {
    Self {
      history_lines: Vec::new(),
      cwd,
      last_exit_code: None,
    }
  }
}

/// Simple message structure for conversation history
#[derive(Debug, Clone)]
pub struct ChatMessage {
  pub role: String,
  pub content: String,
}

impl ChatMessage {
  pub fn system(content: impl Into<String>) -> Self {
    Self {
      role: "system".to_string(),
      content: content.into(),
    }
  }

  pub fn user(content: impl Into<String>) -> Self {
    Self {
      role: "user".to_string(),
      content: content.into(),
    }
  }

  pub fn assistant(content: impl Into<String>) -> Self {
    Self {
      role: "assistant".to_string(),
      content: content.into(),
    }
  }
}
