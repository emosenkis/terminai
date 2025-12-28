// TERMIN.AI: High-level AG-UI client
// Provides clean interface for chat operations using the transport layer
//
// This client handles:
// - Sending messages to the Python LLM agent
// - Receiving streaming responses via SSE
// - Managing conversation state
// - Tool calls and results

use anyhow::{Context, Result};
use futures::StreamExt;
use futures::stream::Stream;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use tokio::io::AsyncBufReadExt;

use crate::llm_old::ag_ui_transport::AgUiTransport;
use crate::llm_subprocess::LlmSubprocessConfig;

/// Message role in conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
  User,
  Assistant,
  System,
}

/// A chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
  pub role: Role,
  pub content: String,
}

/// Terminal context for the agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalContext {
  /// Recent terminal history lines
  pub history_lines: Vec<String>,
  /// Current working directory
  pub cwd: String,
  /// Last command exit code
  pub last_exit_code: Option<i32>,
}

/// Request to start a chat conversation
#[derive(Debug, Serialize)]
struct ChatRequest {
  message: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  context: Option<TerminalContext>,
}

/// Stream event types from AG-UI
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
  /// Text chunk from the assistant
  TextChunk { content: String },
  /// Tool call requested
  ToolCall {
    tool: String,
    args: serde_json::Value,
  },
  /// Tool result provided
  ToolResult { result: serde_json::Value },
  /// Stream complete
  Done,
  /// Error occurred
  Error { message: String },
}

/// High-level AG-UI client
///
/// This provides a clean interface for chatting with the Python LLM agent,
/// abstracting away the HTTP + SSE mechanics handled by the transport layer.
pub struct AgUiClient {
  transport: AgUiTransport,
  http_client: Client,
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

    let transport = AgUiTransport::spawn(config).await?;
    let http_client = Client::new();

    Ok(Self {
      transport,
      http_client,
      provider: provider.into(),
      model: model.into(),
    })
  }

  /// Send a chat message and get a complete response
  ///
  /// This is a non-streaming version that waits for the full response.
  /// Uses AG-UI protocol with forwardedProps for provider/model configuration.
  ///
  /// # Arguments
  /// * `message` - User message to send
  /// * `context` - Optional terminal context
  ///
  /// # Returns
  /// Complete assistant response
  pub async fn chat(
    &self,
    message: impl Into<String>,
    context: Option<TerminalContext>,
  ) -> Result<String> {
    let message = message.into();
    log::debug!("Sending AG-UI chat message: {}", message);

    use crate::llm_old::ag_ui_protocol::RunAgentInput;

    // Create AG-UI RunAgentInput with forwardedProps
    let mut run_input =
      RunAgentInput::new(self.provider.clone(), self.model.clone())
        .with_user_message(message);

    // TODO: Add context to the request (needs to be added to RunAgentInput)

    let url = self.transport.base_url().to_string();

    let response = self
      .http_client
      .post(&url)
      .headers(self.transport.headers().clone())
      .header("Accept", "text/event-stream")
      .json(&run_input)
      .send()
      .await
      .context("Failed to send AG-UI request")?;

    if !response.status().is_success() {
      anyhow::bail!(
        "AG-UI request failed with status {}: {}",
        response.status(),
        response.text().await.unwrap_or_default()
      );
    }

    // For non-streaming, collect all text chunks from SSE
    let mut full_response = String::new();
    let stream = response.bytes_stream();
    let reader = tokio_util::io::StreamReader::new(stream.map(|result| {
      result.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }));

    let mut lines = reader.lines();
    while let Some(line) = lines.next_line().await? {
      if line.starts_with("data: ") {
        let data = &line[6..];
        if let Ok(event) = serde_json::from_str::<StreamEvent>(data) {
          match event {
            StreamEvent::TextChunk { content } => {
              full_response.push_str(&content);
            }
            StreamEvent::Done => break,
            StreamEvent::Error { message } => {
              anyhow::bail!("AG-UI error: {}", message);
            }
            _ => {}
          }
        }
      }
    }

    Ok(full_response)
  }

  /// Send a chat message and get a streaming response
  ///
  /// Returns a stream of events (text chunks, tool calls, etc.)
  /// Uses AG-UI protocol with forwardedProps for provider/model configuration.
  ///
  /// # Arguments
  /// * `message` - User message to send
  /// * `context` - Optional terminal context
  ///
  /// # Returns
  /// Stream of AG-UI events
  pub async fn chat_stream(
    &self,
    message: impl Into<String>,
    context: Option<TerminalContext>,
  ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
    let message = message.into();
    log::debug!("Sending AG-UI streaming chat message: {}", message);

    use crate::llm_old::ag_ui_protocol::RunAgentInput;

    // Create AG-UI RunAgentInput with forwardedProps
    let run_input =
      RunAgentInput::new(self.provider.clone(), self.model.clone())
        .with_user_message(message);

    let url = self.transport.base_url().to_string();

    let response = self
      .http_client
      .post(&url)
      .headers(self.transport.headers().clone())
      .header("Accept", "text/event-stream")
      .json(&run_input)
      .send()
      .await
      .context("Failed to send AG-UI streaming request")?;

    if !response.status().is_success() {
      anyhow::bail!(
        "AG-UI streaming request failed with status {}: {}",
        response.status(),
        response.text().await.unwrap_or_default()
      );
    }

    // Convert the response body into a stream of SSE events
    let stream = response.bytes_stream();
    let reader = tokio_util::io::StreamReader::new(stream.map(|result| {
      result.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }));

    let lines = reader.lines();
    let event_stream = async_stream::stream! {
        let mut lines = lines;
        while let Ok(Some(line)) = lines.next_line().await {
            // SSE format: "data: {...json...}"
            if let Some(json_str) = line.strip_prefix("data: ") {
                match serde_json::from_str::<StreamEvent>(json_str) {
                    Ok(event) => yield Ok(event),
                    Err(e) => {
                        log::warn!("Failed to parse SSE event: {}", e);
                        yield Err(anyhow::anyhow!("Failed to parse event: {}", e));
                    }
                }
            }
        }
    };

    Ok(Box::pin(event_stream))
  }

  /// Check if the subprocess is still running
  pub async fn is_running(&self) -> bool {
    self.transport.is_subprocess_running().await
  }

  /// Shutdown the client and Python subprocess gracefully
  pub async fn shutdown(self) -> Result<()> {
    log::info!("Shutting down AG-UI client");
    self.transport.shutdown().await
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

  #[tokio::test]
  #[cfg_attr(not(feature = "ollama-tests"), ignore)]
  async fn test_chat_basic() {
    // Configure environment for Ollama endpoint
    // SAFETY: Setting environment variables in tests before spawning any threads
    unsafe {
      std::env::set_var("OLLAMA_BASE_URL", "http://localhost:11434");
    }

    let config = LlmSubprocessConfig::for_testing();
    let client = AgUiClient::spawn(config, "ollama", "functiongemma")
      .await
      .expect("Failed to spawn client");

    let response = client
      .chat("What is 2+2? Answer with just the number.", None)
      .await
      .expect("Failed to get chat response");

    assert!(!response.is_empty());
    println!("Ollama response: {}", response);

    client.shutdown().await.expect("Failed to shutdown client");
  }
}
