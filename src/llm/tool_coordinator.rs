// TERMIN.AI: Tool execution coordinator
//
// Coordinates tool execution between the LLM subscriber and the application layer.
// Handles the full cycle: receive tool request → execute → submit result → handle continuation

use ag_ui_core::types::message::Message;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::llm::{
  AgUiClient, ChatStreamResponse, CommandSuggestion, ToolExecutionRequest,
  ToolExecutor,
};

/// Tool coordinator that manages the tool execution lifecycle
pub struct ToolCoordinator {
  client: Arc<AgUiClient>,
  executor: ToolExecutor,
  message_history: Arc<Mutex<Vec<Message>>>,
  command_suggestions: Arc<Mutex<Vec<CommandSuggestion>>>,
}

impl ToolCoordinator {
  pub fn new(
    client: Arc<AgUiClient>,
    executor: ToolExecutor,
    message_history: Arc<Mutex<Vec<Message>>>,
    command_suggestions: Arc<Mutex<Vec<CommandSuggestion>>>,
  ) -> Self {
    Self {
      client,
      executor,
      message_history,
      command_suggestions,
    }
  }

  /// Process a single tool execution request
  ///
  /// Executes the tool and submits the result back to the LLM.
  /// Returns the continued response stream for further processing.
  pub async fn process_tool_request(
    &self,
    request: ToolExecutionRequest,
  ) -> Result<ChatStreamResponse> {
    log::info!(
      "Processing tool request: {} (id: {:?})",
      request.tool_name,
      request.tool_call_id
    );

    // NOTE: We do NOT manually add Assistant messages with tool_calls to history.
    // The AG-UI SDK already adds these through its event stream (via on_new_tool_call).
    // Manually adding them would create duplicates without corresponding tool_results,
    // which violates the Anthropic API protocol.

    // 1. Execute the tool
    let result = self.executor.execute_tool(request).await;

    if result.is_error {
      log::error!(
        "Tool execution failed: {:?} - {}",
        result.tool_call_id,
        result.content
      );
    } else {
      log::info!("Tool execution complete: {:?}", result.tool_call_id);
    }

    // 2. Submit result back to LLM and get continued response
    let continued_response = self
      .client
      .submit_tool_result(Arc::clone(&self.message_history), result, None)
      .await?;

    Ok(continued_response)
  }

  /// Check if there are pending command suggestions
  pub async fn has_suggestions(&self) -> bool {
    let suggestions = self.command_suggestions.lock().await;
    !suggestions.is_empty()
  }

  /// Get the latest command suggestion
  pub async fn get_latest_suggestion(&self) -> Option<CommandSuggestion> {
    let suggestions = self.command_suggestions.lock().await;
    suggestions.last().cloned()
  }

  /// Clear command suggestions
  pub async fn clear_suggestions(&self) {
    let mut suggestions = self.command_suggestions.lock().await;
    suggestions.clear();
  }

  /// Add a message to the conversation history
  pub async fn add_message(&self, message: Message) {
    let mut history = self.message_history.lock().await;
    history.push(message);
  }

  /// Get the current message history
  pub async fn get_history(&self) -> Vec<Message> {
    let history = self.message_history.lock().await;
    history.clone()
  }

  /// Clear the message history
  pub async fn clear_history(&self) {
    let mut history = self.message_history.lock().await;
    history.clear();
  }

  /// Get a reference to the shared message history
  ///
  /// This allows external code (like chat_stream) to pass the history
  /// to subscribers so they can update it when tool calls arrive.
  pub fn get_history_ref(&self) -> Arc<Mutex<Vec<Message>>> {
    Arc::clone(&self.message_history)
  }
}

/// Tool execution task that runs in the background
///
/// Listens for tool requests, executes them, and handles the continued response.
pub async fn run_tool_execution_loop(
  coordinator: Arc<ToolCoordinator>,
  mut tool_rx: tokio::sync::mpsc::UnboundedReceiver<ToolExecutionRequest>,
  event_sender: tokio::sync::mpsc::UnboundedSender<ToolExecutionEvent>,
) {
  log::info!("Tool execution loop started");

  while let Some(request) = tool_rx.recv().await {
    let tool_name = request.tool_name.clone();
    log::info!("Received tool request: {}", tool_name);

    // Process the tool request
    match coordinator.process_tool_request(request).await {
      Ok(continued_response) => {
        // Notify UI that tool execution completed
        let _ = event_sender.send(ToolExecutionEvent::ToolExecuted {
          tool_name: tool_name.clone(),
        });

        // Handle continued response stream
        let mut text_stream = continued_response.text_stream;
        let mut continued_tool_rx = continued_response.tool_rx;

        // Collect continued text response
        let coordinator_clone = Arc::clone(&coordinator);
        let event_sender_clone = event_sender.clone();
        tokio::spawn(async move {
          use futures::StreamExt;

          let mut full_response = String::new();

          while let Some(result) = text_stream.next().await {
            match result {
              Ok(chunk) => {
                full_response.push_str(&chunk);
                let _ = event_sender_clone.send(
                  ToolExecutionEvent::ContinuedTextChunk {
                    chunk: chunk.clone(),
                  },
                );
              }
              Err(e) => {
                log::error!("Error in continued stream: {}", e);
                let _ = event_sender_clone.send(ToolExecutionEvent::Error {
                  message: format!("{}", e),
                });
                break;
              }
            }
          }

          // Add assistant response to history
          if !full_response.is_empty() {
            coordinator_clone
              .add_message(Message::Assistant {
                id: ag_ui_core::types::ids::MessageId::random(),
                content: Some(full_response.clone()),
                name: None,
                tool_calls: None,
              })
              .await;
          }

          // Notify that continued stream is complete
          let _ = event_sender_clone.send(
            ToolExecutionEvent::ContinuedStreamComplete {
              full_response: full_response.clone(),
            },
          );

          log::info!("Continued response stream completed");
        });

        // Handle any additional tool requests from the continued response
        let coordinator_clone = Arc::clone(&coordinator);
        let event_sender_clone = event_sender.clone();
        tokio::spawn(async move {
          while let Some(nested_request) = continued_tool_rx.recv().await {
            log::info!(
              "Received nested tool request: {}",
              nested_request.tool_name
            );

            // Recursively process nested tool requests
            match coordinator_clone.process_tool_request(nested_request).await {
              Ok(_nested_response) => {
                // Could recursively handle more levels if needed
                log::info!("Nested tool request processed");
              }
              Err(e) => {
                log::error!("Error processing nested tool request: {}", e);
                let _ = event_sender_clone.send(ToolExecutionEvent::Error {
                  message: format!("{}", e),
                });
              }
            }
          }
        });
      }
      Err(e) => {
        log::error!("Error processing tool request: {}", e);
        let _ = event_sender.send(ToolExecutionEvent::Error {
          message: format!("{}", e),
        });
      }
    }
  }

  log::info!("Tool execution loop ended");
}

/// Events emitted by the tool execution system
#[derive(Debug, Clone)]
pub enum ToolExecutionEvent {
  /// A tool was executed
  ToolExecuted { tool_name: String },
  /// Continued text chunk from LLM after tool result
  ContinuedTextChunk { chunk: String },
  /// Continued response stream completed
  ContinuedStreamComplete { full_response: String },
  /// Error during tool execution
  Error { message: String },
}
