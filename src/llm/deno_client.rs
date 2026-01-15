// TERMIN.AI: Deno-based LLM client
//
// This module wraps the embedded Deno runtime for LLM communication.
// It provides a compatible interface to replace the Python AG-UI client.
//
// Note: The DenoAgent contains a JsRuntime which is !Send, so we use
// a channel-based architecture where the agent runs in its own task.

use crate::deno::runtime::DenoAgent;
use crate::deno::types::{
  ChatOptions as DenoChatOptions, StreamMessage, TerminalContext as DenoTerminalContext,
};
use crate::llm::tool_executor::ToolExecutor;
use crate::llm::TerminalContext;
use anyhow::{Context, Result};
use futures::stream::Stream;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::UnboundedReceiverStream;

/// Response from chat_stream containing both text stream and tool notifications
pub struct DenoChatStreamResponse {
  /// Stream of text chunks from the LLM
  pub text_stream: Pin<Box<dyn Stream<Item = Result<String>> + Send>>,
  /// Receiver for tool call notifications (for UI feedback)
  /// Note: In the Deno architecture, tools are executed internally,
  /// so this is for observation only, not execution coordination.
  pub tool_notifications: mpsc::UnboundedReceiver<ToolCallNotification>,
}

/// Notification about a tool call (for UI feedback)
#[derive(Debug, Clone)]
pub struct ToolCallNotification {
  pub tool_name: String,
  pub tool_input: serde_json::Value,
}

/// Request to the Deno agent task
enum DenoRequest {
  Chat {
    options: DenoChatOptions,
    text_tx: mpsc::UnboundedSender<Result<String>>,
    tool_tx: mpsc::UnboundedSender<ToolCallNotification>,
    done_tx: oneshot::Sender<Result<()>>,
  },
  IsLoaded {
    reply_tx: oneshot::Sender<bool>,
  },
  Version {
    reply_tx: oneshot::Sender<Result<String>>,
  },
  Shutdown,
}

/// Deno-based LLM client for Termin.AI
///
/// This wraps the embedded Deno runtime with the TypeScript agent.
/// Tools are executed internally via Rust ops, not through HTTP.
///
/// The actual DenoAgent runs in a dedicated task because JsRuntime is !Send.
/// This handle communicates with it via channels.
pub struct DenoLlmClient {
  request_tx: mpsc::UnboundedSender<DenoRequest>,
  provider: String,
  model: String,
}

impl DenoLlmClient {
  /// Create a new Deno LLM client
  ///
  /// # Arguments
  /// * `tool_executor` - Tool executor for handling tool calls
  /// * `provider` - LLM provider name (e.g., "anthropic")
  /// * `model` - Model name (e.g., "claude-sonnet-4-5")
  ///
  /// # Returns
  /// Configured client ready for chat operations
  pub async fn new(
    tool_executor: Arc<ToolExecutor>,
    provider: impl Into<String>,
    model: impl Into<String>,
  ) -> Result<Self> {
    log::info!("Creating Deno LLM client");

    let provider = provider.into();
    let model = model.into();

    // Create channel for requests to the agent task
    let (request_tx, request_rx) = mpsc::unbounded_channel();

    // Spawn the agent task using spawn_local via a LocalSet
    // We need to spawn on a dedicated thread because JsRuntime is !Send
    let tool_executor_clone = tool_executor.clone();
    std::thread::spawn(move || {
      let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime for Deno agent");

      rt.block_on(async move {
        run_deno_agent_task(tool_executor_clone, request_rx).await;
      });
    });

    Ok(Self {
      request_tx,
      provider,
      model,
    })
  }

  /// Get the API key for the configured provider
  fn get_api_key(&self) -> Result<String> {
    match self.provider.as_str() {
      "anthropic" => {
        std::env::var("ANTHROPIC_API_KEY").context(
          "ANTHROPIC_API_KEY environment variable not set. \
                     Please set it to use the Anthropic provider.",
        )
      }
      "openai" => {
        std::env::var("OPENAI_API_KEY").context(
          "OPENAI_API_KEY environment variable not set. \
                     Please set it to use the OpenAI provider.",
        )
      }
      other => Err(anyhow::anyhow!(
        "Unsupported provider: {}. Currently only 'anthropic' is supported with the Deno backend.",
        other
      )),
    }
  }

  /// Send a chat message and get a streaming response
  ///
  /// # Arguments
  /// * `message` - User message to send
  /// * `terminal_context` - Optional terminal context
  ///
  /// # Returns
  /// DenoChatStreamResponse containing text stream and tool notifications
  pub async fn chat_stream(
    &self,
    message: impl Into<String>,
    terminal_context: Option<&TerminalContext>,
  ) -> Result<DenoChatStreamResponse> {
    let message_str = message.into();
    log::debug!("Sending Deno chat message: {}", message_str);

    // Get API key
    let api_key = self.get_api_key()?;

    // Convert terminal context to Deno types
    let deno_context = terminal_context.map(|ctx| DenoTerminalContext {
      history_lines: Some(ctx.history_lines.clone()),
      cwd: ctx.cwd.clone(),
      last_exit_code: ctx.last_exit_code,
      os_info: ctx.os_info.clone(),
      shell: ctx.shell.clone(),
    });

    // Build chat options
    let options = DenoChatOptions {
      message: message_str,
      model: self.model.clone(),
      provider: self.provider.clone(),
      api_key: Some(api_key),
      system_prompt: None,
      terminal_context: deno_context,
      max_turns: None,
      max_budget_usd: None,
    };

    // Create channels for responses
    let (text_tx, text_rx) = mpsc::unbounded_channel();
    let (tool_tx, tool_rx) = mpsc::unbounded_channel();
    let (done_tx, done_rx) = oneshot::channel();

    // Send request to agent task
    self
      .request_tx
      .send(DenoRequest::Chat {
        options,
        text_tx,
        tool_tx,
        done_tx,
      })
      .map_err(|_| anyhow::anyhow!("Deno agent task has shut down"))?;

    // Wait for initialization to complete (stream starts)
    // The done_rx will complete when the stream processing is done or errors
    tokio::spawn(async move {
      let _ = done_rx.await;
    });

    // Return response struct
    Ok(DenoChatStreamResponse {
      text_stream: Box::pin(UnboundedReceiverStream::new(text_rx)),
      tool_notifications: tool_rx,
    })
  }

  /// Get the provider name
  pub fn provider(&self) -> &str {
    &self.provider
  }

  /// Get the model name
  pub fn model(&self) -> &str {
    &self.model
  }

  /// Check if the Deno agent is loaded and ready
  pub async fn is_ready(&self) -> bool {
    let (reply_tx, reply_rx) = oneshot::channel();
    if self
      .request_tx
      .send(DenoRequest::IsLoaded { reply_tx })
      .is_err()
    {
      return false;
    }
    reply_rx.await.unwrap_or(false)
  }

  /// Get the agent version
  pub async fn version(&self) -> Result<String> {
    let (reply_tx, reply_rx) = oneshot::channel();
    self
      .request_tx
      .send(DenoRequest::Version { reply_tx })
      .map_err(|_| anyhow::anyhow!("Deno agent task has shut down"))?;

    reply_rx
      .await
      .map_err(|_| anyhow::anyhow!("Deno agent task did not respond"))?
  }
}

impl Drop for DenoLlmClient {
  fn drop(&mut self) {
    // Send shutdown request to agent task
    let _ = self.request_tx.send(DenoRequest::Shutdown);
  }
}

/// Run the Deno agent in a dedicated task
async fn run_deno_agent_task(
  tool_executor: Arc<ToolExecutor>,
  mut request_rx: mpsc::UnboundedReceiver<DenoRequest>,
) {
  // Create the DenoAgent
  let mut agent = match DenoAgent::new(tool_executor).await {
    Ok(agent) => agent,
    Err(e) => {
      log::error!("Failed to create Deno agent: {}", e);
      return;
    }
  };

  log::info!("Deno agent task started");

  // Process requests
  while let Some(request) = request_rx.recv().await {
    match request {
      DenoRequest::Chat {
        options,
        text_tx,
        tool_tx,
        done_tx,
      } => {
        // Process chat request
        match agent.chat_stream(options).await {
          Ok(stream) => {
            use futures::StreamExt;

            let mut stream = stream;
            while let Some(result) = stream.next().await {
              match result {
                Ok(msg) => match msg {
                  StreamMessage::Text { content } => {
                    if !content.is_empty() {
                      let _ = text_tx.send(Ok(content));
                    }
                  }
                  StreamMessage::ToolCall {
                    tool_name,
                    tool_input,
                  } => {
                    log::debug!("Tool call: {} with {:?}", tool_name, tool_input);
                    let _ = tool_tx.send(ToolCallNotification {
                      tool_name,
                      tool_input,
                    });
                  }
                  StreamMessage::Result {
                    is_error,
                    result,
                    errors,
                    ..
                  } => {
                    if is_error {
                      let error_msg = errors
                        .and_then(|e| e.first().cloned())
                        .unwrap_or_else(|| "Unknown error".to_string());
                      log::error!("Chat error: {}", error_msg);
                      let _ = text_tx.send(Err(anyhow::anyhow!(error_msg)));
                    } else {
                      log::debug!(
                        "Chat completed: {:?}",
                        result.unwrap_or_else(|| "success".to_string())
                      );
                    }
                  }
                },
                Err(e) => {
                  log::error!("Stream error: {}", e);
                  let _ = text_tx.send(Err(e));
                }
              }
            }
            let _ = done_tx.send(Ok(()));
          }
          Err(e) => {
            log::error!("Failed to start chat stream: {}", e);
            let _ = text_tx.send(Err(anyhow::anyhow!("{}", e)));
            let _ = done_tx.send(Err(e));
          }
        }
      }
      DenoRequest::IsLoaded { reply_tx } => {
        let _ = reply_tx.send(agent.is_loaded());
      }
      DenoRequest::Version { reply_tx } => {
        let _ = reply_tx.send(agent.version());
      }
      DenoRequest::Shutdown => {
        log::info!("Deno agent task shutting down");
        break;
      }
    }
  }

  log::info!("Deno agent task ended");
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::command::{CommandExecutor, SafetyValidator};
  use crate::llm::tool_executor::ToolExecutionContext;
  use tokio::sync::Mutex;

  fn create_test_tool_executor() -> Arc<ToolExecutor> {
    let context = ToolExecutionContext {
      vt_parser: None,
      command_suggestions: Arc::new(Mutex::new(Vec::new())),
      command_executor: CommandExecutor::new(),
      safety_validator: SafetyValidator::new(),
    };
    Arc::new(ToolExecutor::new(context))
  }

  #[tokio::test]
  async fn test_deno_client_creation() {
    let tool_executor = create_test_tool_executor();
    let result =
      DenoLlmClient::new(tool_executor, "anthropic", "claude-sonnet-4-5").await;

    assert!(
      result.is_ok(),
      "Failed to create Deno client: {:?}",
      result.err()
    );

    let client = result.unwrap();
    // Give the agent thread time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    assert!(client.is_ready().await);
    assert_eq!(client.provider(), "anthropic");
    assert_eq!(client.model(), "claude-sonnet-4-5");
  }

  #[tokio::test]
  async fn test_deno_client_version() {
    let tool_executor = create_test_tool_executor();
    let client =
      DenoLlmClient::new(tool_executor, "anthropic", "claude-sonnet-4-5")
        .await
        .unwrap();

    // Give the agent thread time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let version = client.version().await;
    assert!(version.is_ok(), "Failed to get version: {:?}", version.err());
    println!("Client version: {}", version.unwrap());
  }

  #[tokio::test]
  async fn test_deno_client_without_api_key() {
    let tool_executor = create_test_tool_executor();
    let client =
      DenoLlmClient::new(tool_executor, "anthropic", "claude-sonnet-4-5")
        .await
        .unwrap();

    // Remove API key from environment for this test
    std::env::remove_var("ANTHROPIC_API_KEY");

    let result = client.chat_stream("Hello", None).await;

    // Should fail because API key is not set
    assert!(result.is_err());
    let error = result.unwrap_err().to_string();
    assert!(
      error.contains("ANTHROPIC_API_KEY"),
      "Expected API key error, got: {}",
      error
    );
  }
}
