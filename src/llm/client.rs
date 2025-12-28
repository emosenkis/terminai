// TERMIN.AI: AG-UI client using official ag-ui-client SDK
//
// This module wraps the official AG-UI Rust client with subprocess management.
// It provides a clean interface for chatting with the Python LLM agent.

use ag_ui_client::agent::RunAgentParams;
use ag_ui_client::{Agent, HttpAgent};
use ag_ui_core::types::context::Context;
use ag_ui_core::types::message::Message;
use ag_ui_core::types::tool::Tool;
use anyhow::Result;
use futures::stream::Stream;
use serde_json::json;
use std::pin::Pin;
use std::sync::Arc;

use crate::llm::subscriber::StreamingSubscriber;
use crate::llm_subprocess::{LlmSubprocess, LlmSubprocessConfig};

/// High-level AG-UI client for Termin.AI
///
/// This wraps the official AG-UI HttpAgent with subprocess lifecycle management
/// and provides Termin.AI-specific defaults (tools, context, etc.)
pub struct AgUiClient {
  http_agent: Arc<HttpAgent>,
  pub(crate) subprocess: LlmSubprocess,
  provider: String,
  model: String,
}

impl AgUiClient {
  /// Create a new AG-UI client by spawning the Python subprocess
  ///
  /// # Arguments
  /// * `config` - Subprocess configuration
  /// * `provider` - LLM provider name (e.g., "ollama", "anthropic")
  /// * `model` - Model name (e.g., "functiongemma", "claude-sonnet-4-5")
  ///
  /// # Returns
  /// Configured client ready for chat operations
  pub async fn spawn(
    config: LlmSubprocessConfig,
    provider: impl Into<String>,
    model: impl Into<String>,
  ) -> Result<Self> {
    log::info!("Creating AG-UI client");

    // Spawn the subprocess
    let subprocess = LlmSubprocess::spawn(config).await?;

    // Create HTTP agent using official SDK
    let http_agent = HttpAgent::builder()
      .with_url_str(subprocess.base_url())?
      .with_header("x-ag-ui-secret", subprocess.secret())?
      .build()?;

    Ok(Self {
      http_agent: Arc::new(http_agent),
      subprocess,
      provider: provider.into(),
      model: model.into(),
    })
  }

  /// Send a chat message and get a streaming text response
  ///
  /// Uses the official SDK's subscriber pattern to stream text chunks
  ///
  /// # Arguments
  /// * `message` - User message to send
  /// * `context_items` - Optional terminal context items
  ///
  /// # Returns
  /// Stream of text chunks
  pub async fn chat_stream(
    &self,
    message: impl Into<String>,
    context_items: Option<Vec<Context>>,
  ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
    use tokio::sync::mpsc;
    use tokio_stream::wrappers::UnboundedReceiverStream;

    let message_str = message.into();
    log::debug!("Sending AG-UI streaming chat message: {}", message_str);

    // Create channel for streaming text
    let (tx, rx) = mpsc::unbounded_channel();

    // Create subscriber
    let subscriber = StreamingSubscriber::new(tx.clone());

    // Build RunAgentParams (using public fields in v0.1)
    let params = RunAgentParams {
      run_id: None,
      tools: Some(Self::default_tools()),
      context: context_items,
      forwarded_props: Some(json!({
          "provider": self.provider,
          "model": self.model,
      })),
      messages: vec![Message::User {
        id: ag_ui_core::types::ids::MessageId::random(),
        content: message_str,
        name: None,
      }],
      state: serde_json::Value::Null,
    };

    // Clone agent for background task
    let agent = Arc::clone(&self.http_agent);

    // Spawn background task to run agent
    tokio::spawn(async move {
      if let Err(e) = agent.run_agent(&params, [subscriber]).await {
        // Send error if agent fails
        let _ = tx.send(Err(anyhow::anyhow!("{}", e)));
      }
      // Channel closes when tx is dropped
    });

    // Return receiver as stream
    let stream = UnboundedReceiverStream::new(rx);
    Ok(Box::pin(stream))
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

  /// Check if the subprocess is still running
  pub async fn is_running(&self) -> bool {
    self.subprocess.is_running().await
  }

  /// Get the base URL of the Python subprocess
  ///
  /// Primarily for testing and debugging.
  pub fn base_url(&self) -> &str {
    self.subprocess.base_url()
  }

  /// Shutdown the client and Python subprocess gracefully
  pub async fn shutdown(self) -> Result<()> {
    log::info!("Shutting down AG-UI client");
    self.subprocess.shutdown().await
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn test_client_lifecycle() {
    let config = LlmSubprocessConfig::for_testing();
    let client = AgUiClient::spawn(config, "ollama", "functiongemma")
      .await
      .expect("Failed to spawn client");

    assert!(client.is_running().await);

    client.shutdown().await.expect("Failed to shutdown client");
  }
}
