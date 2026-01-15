/**
 * Shared types between Rust and TypeScript
 * These should match typescript/agent/types.ts
 */
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalContext {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub history_lines: Option<Vec<String>>,
  pub cwd: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub last_exit_code: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub os_info: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub shell: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatOptions {
  pub message: String,
  pub model: String,
  pub provider: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub api_key: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub system_prompt: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub terminal_context: Option<TerminalContext>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub max_turns: Option<u32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub max_budget_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestCommandArgs {
  pub command: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub explanation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadScrollbackArgs {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub num_lines: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum StreamMessage {
  #[serde(rename = "text")]
  Text { content: String },

  #[serde(rename = "tool_call")]
  ToolCall {
    #[serde(rename = "toolName")]
    tool_name: String,
    #[serde(rename = "toolInput")]
    tool_input: serde_json::Value,
  },

  #[serde(rename = "result")]
  Result {
    #[serde(rename = "isError")]
    is_error: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    errors: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: Option<TokenUsage>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "totalCostUsd")]
    total_cost_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "durationMs")]
    duration_ms: Option<u64>,
  },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
  pub input_tokens: u32,
  pub output_tokens: u32,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub cache_read_input_tokens: Option<u32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub cache_creation_input_tokens: Option<u32>,
}

impl StreamMessage {
  /// Check if this is a text message
  pub fn is_text(&self) -> bool {
    matches!(self, StreamMessage::Text { .. })
  }

  /// Check if this is a result message
  pub fn is_result(&self) -> bool {
    matches!(self, StreamMessage::Result { .. })
  }

  /// Get text content if this is a text message
  pub fn as_text(&self) -> Option<&str> {
    match self {
      StreamMessage::Text { content } => Some(content.as_str()),
      _ => None,
    }
  }
}

/// HTTP fetch options for the op_fetch operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchOptions {
  pub method: String,
  pub headers: HashMap<String, String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub body: Option<String>,
}

/// HTTP fetch response from the op_fetch operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchResponse {
  pub status: u16,
  pub body: String,
}
