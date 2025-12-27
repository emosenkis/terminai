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
}

impl AgUiClient {
  /// Create a new AG-UI client by spawning the Python subprocess
  ///
  /// # Arguments
  /// * `config` - Subprocess configuration
  ///
  /// # Returns
  /// Configured client ready for chat operations
  pub async fn spawn(config: LlmSubprocessConfig) -> Result<Self> {
    log::info!("Creating AG-UI client");

    let transport = AgUiTransport::spawn(config).await?;
    let http_client = Client::new();

    Ok(Self {
      transport,
      http_client,
    })
  }

  /// Send a chat message and get a complete response
  ///
  /// This is a non-streaming version that waits for the full response.
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
    log::debug!("Sending chat message: {}", message);

    let url = format!("{}/chat", self.transport.base_url());
    let request = ChatRequest { message, context };

    let response = self
      .http_client
      .post(&url)
      .headers(self.transport.headers().clone())
      .json(&request)
      .send()
      .await
      .context("Failed to send chat request")?;

    if !response.status().is_success() {
      anyhow::bail!(
        "Chat request failed with status {}: {}",
        response.status(),
        response.text().await.unwrap_or_default()
      );
    }

    #[derive(Deserialize)]
    struct ChatResponse {
      response: String,
    }

    let chat_response: ChatResponse = response
      .json()
      .await
      .context("Failed to parse chat response")?;

    Ok(chat_response.response)
  }

  /// Send a chat message and get a streaming response
  ///
  /// Returns a stream of events (text chunks, tool calls, etc.)
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
    log::debug!("Sending streaming chat message: {}", message);

    let url = format!("{}/chat/stream", self.transport.base_url());
    let request = ChatRequest { message, context };

    let response = self
      .http_client
      .post(&url)
      .headers(self.transport.headers().clone())
      .json(&request)
      .send()
      .await
      .context("Failed to send streaming chat request")?;

    if !response.status().is_success() {
      anyhow::bail!(
        "Streaming chat request failed with status {}: {}",
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
  #[ignore] // Requires Python environment
  async fn test_client_lifecycle() {
    let config = LlmSubprocessConfig::default();
    let client = AgUiClient::spawn(config)
      .await
      .expect("Failed to spawn client");

    assert!(client.is_running().await);

    client.shutdown().await.expect("Failed to shutdown client");
  }

  #[tokio::test]
  #[ignore] // Requires Python environment and API keys
  async fn test_chat_basic() {
    let config = LlmSubprocessConfig::default();
    let client = AgUiClient::spawn(config)
      .await
      .expect("Failed to spawn client");

    let response = client
      .chat("Hello, who are you?", None)
      .await
      .expect("Failed to get chat response");

    assert!(!response.is_empty());
    println!("Response: {}", response);

    client.shutdown().await.expect("Failed to shutdown client");
  }
}
