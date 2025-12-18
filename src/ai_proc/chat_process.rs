use anyhow::Result;
use std::sync::Arc;

use crate::command::{CommandParser, RiskLevel, SafetyValidator};
use crate::llm::{ChatMessage, LLMClient, Provider};
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
  llm_client: Arc<LLMClient>,
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
}

#[derive(Debug, Clone)]
pub struct PendingCommand {
  pub command: String,
  pub risk_level: RiskLevel,
  pub target_process: Option<usize>,
}

impl AIChatProcess {
  /// Create a new AI chat process
  pub async fn new(provider: Provider, model: Option<String>) -> Result<Self> {
    Self::new_with_endpoint(provider, model, None).await
  }

  /// Create a new AI chat process with custom endpoint
  pub async fn new_with_endpoint(
    provider: Provider,
    model: Option<String>,
    endpoint: Option<String>,
  ) -> Result<Self> {
    let llm_client =
      Arc::new(LLMClient::new_with_endpoint(provider, model, endpoint).await?);

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
    context: crate::llm::TerminalContext,
  ) -> Result<
    std::pin::Pin<
      Box<dyn futures::stream::Stream<Item = Result<String>> + Send>,
    >,
  > {
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

    // Filter sensitive information from context
    let filtered_context = crate::llm::TerminalContext {
      history_lines: self.privacy_filter.filter_lines(&context.history_lines),
      cwd: context.cwd,
      last_exit_code: context.last_exit_code,
    };

    // Convert conversation to ChatMessage format
    let history: Vec<ChatMessage> = self
      .conversation
      .iter()
      .filter_map(|msg| match msg.role {
        MessageRole::User => Some(ChatMessage::user(msg.content.clone())),
        MessageRole::Assistant => {
          Some(ChatMessage::assistant(msg.content.clone()))
        }
        MessageRole::System => None, // System messages handled separately
      })
      .collect();

    // Send to LLM with streaming
    self
      .llm_client
      .send_message_stream(&user_message, &filtered_context, &history)
      .await
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
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_activation() {
    let mut process = AIChatProcess {
      // Create a mock LLM client for testing
      llm_client: Arc::new(
        futures::executor::block_on(LLMClient::new(Provider::Anthropic, None))
          .expect("Failed to create test LLM client"),
      ),
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
    };

    assert!(!process.is_active());

    process.activate();
    assert!(process.is_active());

    process.deactivate();
    assert!(!process.is_active());
  }
}
