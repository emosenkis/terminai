// TERMIN.AI: AG-UI protocol types for communication with Python subprocess

use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use uuid::Uuid;

use crate::llm::TerminAIForwardedProps;

/// AG-UI RunAgentInput structure
/// Matches the structure expected by Pydantic AI's AGUIAdapter
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunAgentInput {
  pub thread_id: String,
  pub run_id: String,
  pub messages: Vec<Message>,
  pub tools: Vec<Tool>,
  pub context: Vec<Context>,
  #[serde(default)]
  pub state: JsonValue,
  pub forwarded_props: TerminAIForwardedProps,
}

/// AG-UI Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
  pub name: String,
  pub description: String,
  pub parameters: JsonValue,
}

/// AG-UI Context item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
  pub description: String,
  pub value: String,
}

impl RunAgentInput {
  pub fn new(provider: String, model: String) -> Self {
    Self {
      thread_id: Uuid::new_v4().to_string(),
      run_id: Uuid::new_v4().to_string(),
      messages: Vec::new(),
      tools: Self::default_tools(),
      context: Vec::new(),
      state: JsonValue::Null,
      forwarded_props: TerminAIForwardedProps::new(provider, model),
    }
  }

  pub fn with_user_message(mut self, content: impl Into<String>) -> Self {
    self.messages.push(Message::User {
      id: Uuid::new_v4().to_string(),
      content: content.into(),
      name: None,
    });
    self
  }

  pub fn with_context(mut self, context: Vec<Context>) -> Self {
    self.context = context;
    self
  }

  /// Default tools provided by the Rust side
  fn default_tools() -> Vec<Tool> {
    vec![
      Tool {
        name: "suggest_command".to_string(),
        description: "Suggest a shell command to execute in the terminal"
          .to_string(),
        parameters: json!({
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "description": "The shell command to suggest"
            },
            "explanation": {
              "type": "string",
              "description": "Brief explanation of what the command does"
            }
          },
          "required": ["command"]
        }),
      },
      Tool {
        name: "read_scrollback".to_string(),
        description: "Read the terminal scrollback history".to_string(),
        parameters: json!({
          "type": "object",
          "properties": {
            "num_lines": {
              "type": "integer",
              "description": "Number of lines to read from scrollback (default: 100)",
              "default": 100
            }
          }
        }),
      },
    ]
  }
}

/// AG-UI Message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum Message {
  User {
    id: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
  },
  Assistant {
    id: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
  },
  System {
    id: String,
    content: String,
  },
}
