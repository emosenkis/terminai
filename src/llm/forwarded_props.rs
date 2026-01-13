// TERMIN.AI: AG-UI forwarded properties for LLM configuration

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Forwarded properties sent with each AG-UI request
/// Contains runtime configuration that can change per-request
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TerminAIForwardedProps {
  /// Provider name (e.g., "ollama", "anthropic", "openai")
  pub provider: String,
  /// Model name (e.g., "functiongemma", "claude-sonnet-4-5")
  pub model: String,
  /// Current working directory
  #[serde(skip_serializing_if = "Option::is_none")]
  pub cwd: Option<String>,
  /// Recent terminal history lines
  #[serde(skip_serializing_if = "Option::is_none")]
  pub history_lines: Option<Vec<String>>,
  /// Last command exit code
  #[serde(skip_serializing_if = "Option::is_none")]
  pub last_exit_code: Option<i32>,
  /// Operating system information
  #[serde(skip_serializing_if = "Option::is_none")]
  pub os_info: Option<String>,
  /// User's shell
  #[serde(skip_serializing_if = "Option::is_none")]
  pub shell: Option<String>,
}

impl TerminAIForwardedProps {
  pub fn new(provider: impl Into<String>, model: impl Into<String>) -> Self {
    Self {
      provider: provider.into(),
      model: model.into(),
      cwd: None,
      history_lines: None,
      last_exit_code: None,
      os_info: None,
      shell: None,
    }
  }

  /// Create with terminal context
  pub fn with_context(
    provider: impl Into<String>,
    model: impl Into<String>,
    context: &crate::llm::TerminalContext,
  ) -> Self {
    Self {
      provider: provider.into(),
      model: model.into(),
      cwd: Some(context.cwd.clone()),
      history_lines: Some(context.history_lines.clone()),
      last_exit_code: context.last_exit_code,
      os_info: context.os_info.clone(),
      shell: context.shell.clone(),
    }
  }

  /// Convert to JSON value for use in AG-UI requests
  pub fn to_json(&self) -> JsonValue {
    serde_json::to_value(self).unwrap_or(JsonValue::Null)
  }
}
