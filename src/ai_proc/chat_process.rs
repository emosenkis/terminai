use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::command::{CommandParser, RiskLevel, SafetyValidator};
use crate::llm::{
  AgUiClient, CommandSuggestion, Message as AgUiMessage, TerminalContext,
  ToolCoordinator, ToolExecutionContext, ToolExecutionEvent, ToolExecutor,
};
use crate::llm_subprocess::LlmSubprocessConfig;
use crate::privacy::PrivacyFilter;

/// Message in the chat conversation
#[derive(Debug, Clone)]
pub struct Message {
  pub role: MessageRole,
  pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
  User,
  Assistant,
  System,
}

/// AI chat process state
pub struct AIChatProcess {
  llm_client: Arc<AgUiClient>,
  conversation: Vec<Message>,
  command_parser: CommandParser,
  safety_validator: SafetyValidator,
  privacy_filter: PrivacyFilter,
  active: bool,
  awaiting_approval: Option<PendingCommand>,
  error_message: Option<String>,
  error_scroll_offset: u16,
  is_sending: bool,
  scroll_offset: u16,
  /// Streaming response in progress (not yet in conversation history)
  streaming_response: Option<String>,
  /// Tool execution coordinator
  coordinator: Arc<ToolCoordinator>,
  /// Command suggestions from LLM tool calls
  command_suggestions: Arc<Mutex<Vec<CommandSuggestion>>>,
  /// Tool execution event receiver
  tool_event_rx:
    Option<tokio::sync::mpsc::UnboundedReceiver<ToolExecutionEvent>>,
}

#[derive(Debug, Clone)]
pub struct PendingCommand {
  pub command: String,
  pub risk_level: RiskLevel,
  pub target_process: Option<usize>,
}

impl AIChatProcess {
  /// Create a new AI chat process
  ///
  /// This spawns a Python subprocess running the LLM agent.
  /// Provider and model must be explicitly provided.
  ///
  /// **Deprecated**: Use `new_with_provider()` instead.
  pub async fn new() -> Result<Self> {
    // Default to ollama/functiongemma for backward compatibility
    Self::new_with_provider("ollama".to_string(), "functiongemma".to_string())
      .await
  }

  /// Create a new AI chat process with provider and model
  ///
  /// This spawns a Python subprocess with the specified provider and model.
  pub async fn new_with_provider(
    provider: String,
    model: String,
  ) -> Result<Self> {
    let config = LlmSubprocessConfig::default();
    Self::new_with_config(config, provider, model).await
  }

  /// Create a new AI chat process with custom subprocess configuration
  pub async fn new_with_config(
    config: LlmSubprocessConfig,
    provider: String,
    model: String,
  ) -> Result<Self> {
    let llm_client =
      Arc::new(AgUiClient::spawn(config, provider, model).await?);

    // Create shared state for tool execution
    let command_suggestions = Arc::new(Mutex::new(Vec::new()));
    let message_history = Arc::new(Mutex::new(Vec::new()));

    // Create tool executor
    let tool_context = ToolExecutionContext {
      vt_parser: None, // Will be set later if needed
      command_suggestions: Arc::clone(&command_suggestions),
      command_executor: crate::command::CommandExecutor::new(),
      safety_validator: crate::command::SafetyValidator::new(),
    };
    let tool_executor = ToolExecutor::new(tool_context);

    // Create tool coordinator
    let coordinator = Arc::new(ToolCoordinator::new(
      Arc::clone(&llm_client),
      tool_executor,
      message_history,
      Arc::clone(&command_suggestions),
    ));

    Ok(Self {
      llm_client,
      conversation: Vec::new(),
      command_parser: CommandParser::new(),
      safety_validator: SafetyValidator::new(),
      privacy_filter: PrivacyFilter::new(),
      active: false,
      awaiting_approval: None,
      error_message: None,
      error_scroll_offset: 0,
      is_sending: false,
      scroll_offset: 0,
      streaming_response: None,
      coordinator,
      command_suggestions,
      tool_event_rx: None,
    })
  }

  /// Activate the AI chat interface
  pub fn activate(&mut self) {
    self.active = true;
  }

  /// Deactivate the AI chat interface
  pub fn deactivate(&mut self) {
    self.active = false;
  }

  /// Check if the chat is active
  pub fn is_active(&self) -> bool {
    self.active
  }

  /// Start streaming response (returns stream for processing outside lock)
  pub async fn start_streaming(
    &mut self,
    user_message: &str,
    context: TerminalContext,
  ) -> Result<crate::llm::ChatStreamResponse> {
    use ag_ui_core::types::ids::MessageId;
    use futures::StreamExt;

    if user_message.is_empty() {
      return Err(anyhow::anyhow!("Empty message"));
    }

    // Clear any previous error message and set sending state
    self.error_message = None;
    self.is_sending = true;
    self.streaming_response = Some(String::new());

    // Add user message to conversation
    self.conversation.push(Message {
      role: MessageRole::User,
      content: user_message.to_string(),
    });

    // Add user message to AG-UI message history for tool execution
    self
      .coordinator
      .add_message(AgUiMessage::User {
        id: MessageId::random(),
        content: user_message.to_string(),
        name: None,
      })
      .await;

    // Filter sensitive information from context
    let filtered_context = TerminalContext {
      history_lines: self.privacy_filter.filter_lines(&context.history_lines),
      cwd: context.cwd.clone(),
      last_exit_code: context.last_exit_code,
      os_info: context.os_info.clone(),
      shell: context.shell.clone(),
    };

    // Get streaming response with text and tool requests from AG-UI client
    let response = self
      .llm_client
      .chat_stream(user_message, Some(&filtered_context))
      .await?;

    // Spawn tool execution loop in background
    let coordinator = Arc::clone(&self.coordinator);
    let tool_rx = response.tool_rx;
    let (event_tx, event_rx) =
      tokio::sync::mpsc::unbounded_channel::<ToolExecutionEvent>();

    tokio::spawn(async move {
      crate::llm::run_tool_execution_loop(coordinator, tool_rx, event_tx).await;
    });

    // Store the event receiver for checking tool events
    self.tool_event_rx = Some(event_rx);

    // Return just the text stream in ChatStreamResponse format
    // (tool_rx is consumed by background loop, so create a dummy channel)
    let (_dummy_tool_tx, dummy_tool_rx) =
      tokio::sync::mpsc::unbounded_channel();

    Ok(crate::llm::ChatStreamResponse {
      text_stream: response.text_stream,
      tool_rx: dummy_tool_rx,
    })
  }

  /// Start streaming for continued response after tool execution
  pub fn start_continued_streaming(&mut self) {
    self.is_sending = true;
    self.streaming_response = Some(String::new());
  }

  /// Append a token to the streaming response
  pub fn append_streaming_token(&mut self, token: String) {
    if let Some(ref mut response) = self.streaming_response {
      response.push_str(&token);
    }
  }

  /// Complete the streaming response and add to conversation
  pub fn complete_streaming(&mut self, full_response: String) {
    self.streaming_response = None;
    self.is_sending = false;

    // Add assistant response to conversation
    self.conversation.push(Message {
      role: MessageRole::Assistant,
      content: full_response.clone(),
    });

    // Check for commands in response
    self.check_for_commands(&full_response, None);
  }

  /// Abort streaming due to error
  pub fn abort_streaming(&mut self) {
    self.is_sending = false;
    self.streaming_response = None;
  }

  /// Check the response for commands and set up approval if needed
  fn check_for_commands(
    &mut self,
    response: &str,
    target_process: Option<usize>,
  ) {
    let commands = self.command_parser.extract_commands(response);

    if let Some(command) = commands.first() {
      let risk_level = self.safety_validator.assess_risk(command);

      // All commands require user approval before execution
      self.awaiting_approval = Some(PendingCommand {
        command: command.clone(),
        risk_level,
        target_process,
      });
    }
  }

  /// Get the pending command awaiting approval
  pub fn pending_command(&self) -> Option<&PendingCommand> {
    self.awaiting_approval.as_ref()
  }

  /// Set a command suggestion as pending approval
  pub async fn set_pending_command(&mut self, suggestion: CommandSuggestion) {
    self.awaiting_approval = Some(PendingCommand {
      command: suggestion.command,
      risk_level: suggestion.risk_level,
      target_process: None, // Default to no specific process
    });

    // Clear suggestions after converting to pending
    self.coordinator.clear_suggestions().await;
  }

  /// Approve the pending command
  pub fn approve_command(&mut self) -> Option<PendingCommand> {
    self.awaiting_approval.take()
  }

  /// Reject the pending command
  pub fn reject_command(&mut self) {
    self.awaiting_approval = None;
  }

  /// Get the conversation history
  pub fn conversation(&self) -> &[Message] {
    &self.conversation
  }

  /// Clear the conversation history
  pub fn clear_conversation(&mut self) {
    self.conversation.clear();
    self.awaiting_approval = None;
  }

  /// Get the current error message, if any
  pub fn error_message(&self) -> Option<&str> {
    self.error_message.as_deref()
  }

  /// Set an error message to display to the user
  pub fn set_error(&mut self, message: String) {
    self.error_message = Some(message);
    self.error_scroll_offset = 0; // Reset scroll when new error is set
  }

  /// Clear the current error message
  pub fn clear_error(&mut self) {
    self.error_message = None;
    self.error_scroll_offset = 0; // Reset scroll when error is cleared
  }

  /// Check if a message is currently being sent
  pub fn is_sending(&self) -> bool {
    self.is_sending
  }

  /// Get the error scroll offset
  pub fn error_scroll_offset(&self) -> u16 {
    self.error_scroll_offset
  }

  /// Scroll up in the error dialog
  pub fn error_scroll_up(&mut self, amount: u16) {
    self.error_scroll_offset = self.error_scroll_offset.saturating_add(amount);
  }

  /// Scroll down in the error dialog
  pub fn error_scroll_down(&mut self, amount: u16) {
    self.error_scroll_offset = self.error_scroll_offset.saturating_sub(amount);
  }

  /// Get the current scroll offset
  pub fn scroll_offset(&self) -> u16 {
    self.scroll_offset
  }

  /// Get the streaming response in progress (if any)
  pub fn streaming_response(&self) -> Option<&str> {
    self.streaming_response.as_deref()
  }

  /// Scroll up in the conversation
  pub fn scroll_up(&mut self, amount: u16) {
    self.scroll_offset = self.scroll_offset.saturating_add(amount);
  }

  /// Scroll down in the conversation
  pub fn scroll_down(&mut self, amount: u16) {
    self.scroll_offset = self.scroll_offset.saturating_sub(amount);
  }

  /// Reset scroll to bottom (most recent messages)
  pub fn scroll_to_bottom(&mut self) {
    self.scroll_offset = 0;
  }

  /// Check for pending command suggestions from tool calls
  pub async fn has_command_suggestions(&self) -> bool {
    self.coordinator.has_suggestions().await
  }

  /// Get the latest command suggestion from tool calls
  pub async fn get_latest_suggestion(&self) -> Option<CommandSuggestion> {
    self.coordinator.get_latest_suggestion().await
  }

  /// Clear all command suggestions
  pub async fn clear_suggestions(&mut self) {
    self.coordinator.clear_suggestions().await
  }

  /// Check for tool execution events (non-blocking)
  ///
  /// Returns the next tool event if one is available.
  pub fn try_recv_tool_event(&mut self) -> Option<ToolExecutionEvent> {
    if let Some(ref mut rx) = self.tool_event_rx {
      rx.try_recv().ok()
    } else {
      None
    }
  }

  /// Set the VT parser for scrollback reading
  ///
  /// This should be called after initialization with the shell's VT parser.
  pub async fn set_vt_parser(
    &mut self,
    vt_parser: std::sync::Arc<
      std::sync::RwLock<crate::vt100::Parser<crate::shell::ReplySender>>,
    >,
  ) {
    // Update the coordinator's tool executor context
    // Note: We need to reconstruct the coordinator with the new VT parser
    let command_suggestions = Arc::clone(&self.command_suggestions);
    let message_history =
      Arc::new(Mutex::new(self.coordinator.get_history().await));

    let tool_context = ToolExecutionContext {
      vt_parser: Some(vt_parser),
      command_suggestions: Arc::clone(&command_suggestions),
      command_executor: crate::command::CommandExecutor::new(),
      safety_validator: crate::command::SafetyValidator::new(),
    };
    let tool_executor = ToolExecutor::new(tool_context);

    self.coordinator = Arc::new(ToolCoordinator::new(
      Arc::clone(&self.llm_client),
      tool_executor,
      message_history,
      command_suggestions,
    ));
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn test_activation() {
    use crate::llm_subprocess::LlmSubprocessConfig;

    let config = LlmSubprocessConfig::for_testing();
    let mut process = AIChatProcess::new_with_config(
      config,
      "ollama".to_string(),
      "functiongemma".to_string(),
    )
    .await
    .expect("Failed to create AI chat process");

    assert!(!process.is_active());

    process.activate();
    assert!(process.is_active());

    process.deactivate();
    assert!(!process.is_active());
  }
}
