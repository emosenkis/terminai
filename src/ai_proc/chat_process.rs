use anyhow::Result;
use genai::chat::ChatMessage;
use std::sync::Arc;

use crate::command::{CommandParser, RiskLevel, SafetyValidator};
use crate::llm::{LLMClient, Provider};
use crate::privacy::PrivacyFilter;

use super::context::ContextExtractor;

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
  input_buffer: String,
  context_extractor: ContextExtractor,
  command_parser: CommandParser,
  safety_validator: SafetyValidator,
  privacy_filter: PrivacyFilter,
  active: bool,
  streaming_response: Option<String>,
  awaiting_approval: Option<PendingCommand>,
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
    let llm_client = Arc::new(LLMClient::new(provider, model).await?);

    Ok(Self {
      llm_client,
      conversation: Vec::new(),
      input_buffer: String::new(),
      context_extractor: ContextExtractor::default(),
      command_parser: CommandParser::new(),
      safety_validator: SafetyValidator::new(),
      privacy_filter: PrivacyFilter::new(),
      active: false,
      streaming_response: None,
      awaiting_approval: None,
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

  /// Get the current input buffer
  pub fn input_buffer(&self) -> &str {
    &self.input_buffer
  }

  /// Append text to input buffer
  pub fn append_input(&mut self, text: &str) {
    self.input_buffer.push_str(text);
  }

  /// Delete the last character from input buffer
  pub fn delete_char(&mut self) {
    self.input_buffer.pop();
  }

  /// Clear the input buffer
  pub fn clear_input(&mut self) {
    self.input_buffer.clear();
  }

  /// Send the current input as a message
  pub async fn send_input(
    &mut self,
    proc_views: &[crate::proc::view::ProcView],
    target_process: Option<usize>,
  ) -> Result<()> {
    if self.input_buffer.is_empty() {
      return Ok(());
    }

    let user_message = self.input_buffer.clone();
    self.input_buffer.clear();

    // Add user message to conversation
    self.conversation.push(Message {
      role: MessageRole::User,
      content: user_message.clone(),
    });

    // Extract context
    let cwd = ContextExtractor::get_cwd();
    let context =
      self
        .context_extractor
        .extract_context(proc_views, target_process, cwd);

    // Filter sensitive information from context
    let filtered_context = crate::llm::TerminalContext {
      history_lines: self.privacy_filter.filter_lines(&context.history_lines),
      cwd: context.cwd,
      last_exit_code: context.last_exit_code,
    };

    // Convert conversation to genai format
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

    // Send to LLM
    let response = self
      .llm_client
      .send_message(&user_message, &filtered_context, &history)
      .await?;

    // Add assistant response to conversation
    self.conversation.push(Message {
      role: MessageRole::Assistant,
      content: response.clone(),
    });

    // Check for commands in response
    self.check_for_commands(&response, target_process);

    Ok(())
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

      // Auto-approve safe commands, require approval for others
      if matches!(risk_level, RiskLevel::Caution | RiskLevel::Dangerous) {
        self.awaiting_approval = Some(PendingCommand {
          command: command.clone(),
          risk_level,
          target_process,
        });
      }
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
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_input_buffer() {
    let provider = Provider::Anthropic;
    let mut process = AIChatProcess {
      // Create a mock LLM client for testing
      llm_client: Arc::new(
        futures::executor::block_on(LLMClient::new(Provider::Anthropic, None))
          .expect("Failed to create test LLM client"),
      ),
      conversation: Vec::new(),
      input_buffer: String::new(),
      context_extractor: ContextExtractor::default(),
      command_parser: CommandParser::new(),
      safety_validator: SafetyValidator::new(),
      privacy_filter: PrivacyFilter::new(),
      active: false,
      streaming_response: None,
      awaiting_approval: None,
    };

    process.append_input("hello");
    assert_eq!(process.input_buffer(), "hello");

    process.append_input(" world");
    assert_eq!(process.input_buffer(), "hello world");

    process.delete_char();
    assert_eq!(process.input_buffer(), "hello worl");

    process.clear_input();
    assert_eq!(process.input_buffer(), "");
  }

  #[test]
  fn test_activation() {
    let provider = Provider::Anthropic;
    let mut process = AIChatProcess {
      // Create a mock LLM client for testing
      llm_client: Arc::new(
        futures::executor::block_on(LLMClient::new(Provider::Anthropic, None))
          .expect("Failed to create test LLM client"),
      ),
      conversation: Vec::new(),
      input_buffer: String::new(),
      context_extractor: ContextExtractor::default(),
      command_parser: CommandParser::new(),
      safety_validator: SafetyValidator::new(),
      privacy_filter: PrivacyFilter::new(),
      active: false,
      streaming_response: None,
      awaiting_approval: None,
    };

    assert!(!process.is_active());

    process.activate();
    assert!(process.is_active());

    process.deactivate();
    assert!(!process.is_active());
  }
}
