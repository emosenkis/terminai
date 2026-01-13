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
use crate::llm::tool_executor::ToolExecutionRequest;
use crate::llm::{TerminAIForwardedProps, TerminalContext};
use crate::llm_subprocess::{LlmSubprocess, LlmSubprocessConfig};

/// Response from chat_stream containing both text stream and tool requests
pub struct ChatStreamResponse {
  /// Stream of text chunks from the LLM
  pub text_stream: Pin<Box<dyn Stream<Item = Result<String>> + Send>>,
  /// Receiver for tool execution requests
  pub tool_rx: tokio::sync::mpsc::UnboundedReceiver<ToolExecutionRequest>,
}

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

  /// Send a chat message and get a streaming text response with tool requests
  ///
  /// Uses the official SDK's subscriber pattern to stream text chunks and
  /// capture tool execution requests.
  ///
  /// # Arguments
  /// * `message` - User message to send
  /// * `terminal_context` - Optional terminal context
  ///
  /// # Returns
  /// ChatStreamResponse containing text stream and tool request receiver
  pub async fn chat_stream(
    &self,
    message: impl Into<String>,
    terminal_context: Option<&TerminalContext>,
  ) -> Result<ChatStreamResponse> {
    use tokio::sync::mpsc;
    use tokio_stream::wrappers::UnboundedReceiverStream;

    let message_str = message.into();
    log::debug!("Sending AG-UI streaming chat message: {}", message_str);

    // Create channel for streaming text
    let (tx, rx) = mpsc::unbounded_channel();

    // Create channel for tool execution requests
    let (tool_tx, tool_rx) = mpsc::unbounded_channel();

    // Create subscriber
    let subscriber = StreamingSubscriber::new(tx.clone(), tool_tx);

    // Convert terminal context to AG-UI context items and forwarded props
    let (context_items, forwarded_props) = match terminal_context {
      Some(ctx) => {
        let items = ctx.to_ag_ui_context();
        let props = TerminAIForwardedProps::with_context(
          &self.provider,
          &self.model,
          ctx,
        );
        (Some(items), Some(props.to_json()))
      }
      None => {
        let props = TerminAIForwardedProps::new(&self.provider, &self.model);
        (None, Some(props.to_json()))
      }
    };

    // Build RunAgentParams (using public fields in v0.1)
    let params = RunAgentParams {
      run_id: None,
      tools: Some(Self::default_tools()),
      context: context_items,
      forwarded_props,
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

    // Return both text stream and tool request receiver
    let text_stream = UnboundedReceiverStream::new(rx);
    Ok(ChatStreamResponse {
      text_stream: Box::pin(text_stream),
      tool_rx,
    })
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

  /// Submit tool result and continue conversation
  ///
  /// Creates a NEW HTTP request with tool result message appended to history.
  /// Returns stream of LLM's continued response with tool requests.
  ///
  /// # Arguments
  /// * `messages` - Full message history so far
  /// * `tool_result` - Tool execution result to submit
  /// * `terminal_context` - Optional terminal context
  ///
  /// # Returns
  /// ChatStreamResponse containing continued text stream and tool request receiver
  pub async fn submit_tool_result(
    &self,
    messages: Vec<Message>,
    tool_result: crate::llm::tool_executor::ToolResult,
    terminal_context: Option<&TerminalContext>,
  ) -> Result<ChatStreamResponse> {
    use tokio::sync::mpsc;
    use tokio_stream::wrappers::UnboundedReceiverStream;

    log::info!(
      "Submitting tool result for {:?} (error: {})",
      tool_result.tool_call_id,
      tool_result.is_error
    );

    // Create channel for streaming text
    let (tx, rx) = mpsc::unbounded_channel();

    // Create channel for tool execution requests
    let (tool_tx, tool_rx) = mpsc::unbounded_channel();

    // Create subscriber
    let subscriber = StreamingSubscriber::new(tx.clone(), tool_tx);

    // Append tool result to messages
    let mut updated_messages = messages;

    // Convert is_error boolean to Option<String> error field
    let error = if tool_result.is_error {
      Some("Tool execution failed".to_string())
    } else {
      None
    };

    updated_messages.push(Message::Tool {
      id: ag_ui_core::types::ids::MessageId::random(),
      tool_call_id: tool_result.tool_call_id,
      content: tool_result.content,
      error,
    });

    // Convert terminal context to AG-UI context items and forwarded props
    let (context_items, forwarded_props) = match terminal_context {
      Some(ctx) => {
        let items = ctx.to_ag_ui_context();
        let props = TerminAIForwardedProps::with_context(
          &self.provider,
          &self.model,
          ctx,
        );
        (Some(items), Some(props.to_json()))
      }
      None => {
        let props = TerminAIForwardedProps::new(&self.provider, &self.model);
        (None, Some(props.to_json()))
      }
    };

    // Build RunAgentParams with updated messages
    let params = RunAgentParams {
      run_id: None,
      tools: Some(Self::default_tools()),
      context: context_items,
      forwarded_props,
      messages: updated_messages,
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

    // Return both text stream and tool request receiver
    let text_stream = UnboundedReceiverStream::new(rx);
    Ok(ChatStreamResponse {
      text_stream: Box::pin(text_stream),
      tool_rx,
    })
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
