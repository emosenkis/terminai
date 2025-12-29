// TERMIN.AI: Complete tool execution integration example
//
// This module demonstrates how to wire together all the tool execution components
// in an application. Copy this pattern into your event loop.

use ag_ui_core::types::ids::MessageId;
use ag_ui_core::types::message::Message;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::command::{CommandExecutor, SafetyValidator};
use crate::llm::{
  AgUiClient, ChatStreamResponse, CommandSuggestion, TerminalContext,
  ToolCoordinator, ToolExecutionContext, ToolExecutionEvent, ToolExecutor,
  run_tool_execution_loop,
};
use crate::llm_subprocess::LlmSubprocessConfig;
use crate::shell::ReplySender;

/// Complete integration example
///
/// This shows how to set up and run the full tool execution system.
pub struct ToolIntegrationExample {
  pub client: Arc<AgUiClient>,
  pub coordinator: Arc<ToolCoordinator>,
  pub command_suggestions: Arc<Mutex<Vec<CommandSuggestion>>>,
  pub event_rx: tokio::sync::mpsc::UnboundedReceiver<ToolExecutionEvent>,
}

impl ToolIntegrationExample {
  /// Initialize the complete tool execution system
  pub async fn new(
    provider: String,
    model: String,
    vt_parser: Option<
      Arc<std::sync::RwLock<crate::vt100::Parser<ReplySender>>>,
    >,
  ) -> Result<Self> {
    // 1. Create LLM client
    let config = LlmSubprocessConfig::default();
    let client = Arc::new(AgUiClient::spawn(config, provider, model).await?);

    // 2. Create shared state
    let command_suggestions = Arc::new(Mutex::new(Vec::new()));
    let message_history = Arc::new(Mutex::new(Vec::new()));

    // 3. Create tool executor
    let tool_context = ToolExecutionContext {
      vt_parser,
      command_suggestions: Arc::clone(&command_suggestions),
      command_executor: CommandExecutor::new(),
      safety_validator: SafetyValidator::new(),
    };
    let tool_executor = ToolExecutor::new(tool_context);

    // 4. Create coordinator
    let coordinator = Arc::new(ToolCoordinator::new(
      Arc::clone(&client),
      tool_executor,
      message_history,
      Arc::clone(&command_suggestions),
    ));

    // 5. Create event channel for tool execution notifications
    let (event_tx, event_rx) =
      tokio::sync::mpsc::unbounded_channel::<ToolExecutionEvent>();

    // Return the integration
    Ok(Self {
      client,
      coordinator,
      command_suggestions,
      event_rx,
    })
  }

  /// Start a chat with tool execution enabled
  ///
  /// Returns the text stream and spawns background tool execution.
  pub async fn start_chat(
    &self,
    user_message: String,
    context: Option<TerminalContext>,
  ) -> Result<tokio::sync::mpsc::UnboundedReceiver<String>> {
    // Add user message to history
    self
      .coordinator
      .add_message(Message::User {
        id: MessageId::random(),
        content: user_message.clone(),
        name: None,
      })
      .await;

    // Convert context to AG-UI format
    let context_items = context.map(|c| c.to_ag_ui_context());

    // Start chat stream
    let ChatStreamResponse {
      text_stream,
      tool_rx,
    } = self.client.chat_stream(user_message, context_items).await?;

    // Create channel for UI text updates
    let (text_tx, text_rx) = tokio::sync::mpsc::unbounded_channel();

    // Spawn task to process text stream
    let text_tx_clone = text_tx.clone();
    tokio::spawn(async move {
      use futures::StreamExt;
      let mut text_stream = text_stream;

      while let Some(result) = text_stream.next().await {
        match result {
          Ok(chunk) => {
            let _ = text_tx_clone.send(chunk);
          }
          Err(e) => {
            log::error!("Stream error: {}", e);
            break;
          }
        }
      }
    });

    // Spawn tool execution loop
    let coordinator = Arc::clone(&self.coordinator);
    let (event_tx, _event_rx) =
      tokio::sync::mpsc::unbounded_channel::<ToolExecutionEvent>();

    tokio::spawn(async move {
      run_tool_execution_loop(coordinator, tool_rx, event_tx).await;
    });

    Ok(text_rx)
  }

  /// Check for command suggestions and return the latest
  pub async fn check_suggestions(&self) -> Option<CommandSuggestion> {
    self.coordinator.get_latest_suggestion().await
  }

  /// Clear all command suggestions
  pub async fn clear_suggestions(&self) {
    self.coordinator.clear_suggestions().await
  }
}

/// Example of how to use the integration in an event loop
#[cfg(test)]
mod example_usage {
  use super::*;

  #[tokio::test]
  #[ignore] // This is an example, not a real test
  async fn example_integration() -> Result<()> {
    // Initialize the integration
    let mut integration = ToolIntegrationExample::new(
      "ollama".to_string(),
      "functiongemma".to_string(),
      None, // No VT parser for this example
    )
    .await?;

    // Start a chat
    let mut text_rx = integration
      .start_chat(
        "Please help me list files".to_string(),
        None, // No terminal context for this example
      )
      .await?;

    // Main event loop
    loop {
      tokio::select! {
        // Handle text chunks from LLM
        Some(chunk) = text_rx.recv() => {
          println!("LLM: {}", chunk);
        }

        // Handle tool execution events
        Some(event) = integration.event_rx.recv() => {
          match event {
            ToolExecutionEvent::ToolExecuted { tool_name } => {
              println!("Tool executed: {}", tool_name);

              // Check for command suggestions
              if tool_name == "suggest_command" {
                if let Some(suggestion) = integration.check_suggestions().await {
                  println!("Command suggested: {}", suggestion.command);
                  println!("Risk: {:?}", suggestion.risk_level);

                  // In a real app, show CommandSuggestionModal here
                  // app.modal = Some(CommandSuggestionModal::new(pc, suggestion));

                  // For this example, just clear it
                  integration.clear_suggestions().await;
                }
              }
            }
            ToolExecutionEvent::ContinuedTextChunk { chunk } => {
              println!("Continued: {}", chunk);
            }
            ToolExecutionEvent::ContinuedStreamComplete { full_response } => {
              println!("Response complete: {} chars", full_response.len());
            }
            ToolExecutionEvent::Error { message } => {
              eprintln!("Error: {}", message);
            }
          }
        }

        else => break,
      }
    }

    Ok(())
  }
}
