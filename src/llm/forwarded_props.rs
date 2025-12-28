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
}

impl TerminAIForwardedProps {
  pub fn new(provider: impl Into<String>, model: impl Into<String>) -> Self {
    Self {
      provider: provider.into(),
      model: model.into(),
    }
  }

  /// Convert to JSON value for use in AG-UI requests
  pub fn to_json(&self) -> JsonValue {
    serde_json::to_value(self).unwrap_or(JsonValue::Null)
  }
}
