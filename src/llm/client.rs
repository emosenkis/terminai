use anyhow::{Context, Result};
use genai::Client;
use genai::chat::{ChatMessage, ChatRequest, ChatStreamResponse};
use std::path::PathBuf;

use super::prompts;
use super::providers::Provider;

/// Terminal context passed to the LLM
#[derive(Debug, Clone)]
pub struct TerminalContext {
  pub history_lines: Vec<String>,
  pub cwd: PathBuf,
  pub last_exit_code: Option<i32>,
}

impl TerminalContext {
  pub fn new(
    history_lines: Vec<String>,
    cwd: PathBuf,
    last_exit_code: Option<i32>,
  ) -> Self {
    Self {
      history_lines,
      cwd,
      last_exit_code,
    }
  }

  pub fn empty(cwd: PathBuf) -> Self {
    Self {
      history_lines: Vec::new(),
      cwd,
      last_exit_code: None,
    }
  }
}

/// LLM client for interacting with various AI providers
pub struct LLMClient {
  client: Client,
  provider: Provider,
  model: String,
}

impl LLMClient {
  /// Create a new LLM client
  pub async fn new(provider: Provider, model: Option<String>) -> Result<Self> {
    let model = model.unwrap_or_else(|| provider.default_model().to_string());

    // Initialize genai client
    let client = Client::default();

    Ok(Self {
      client,
      provider,
      model,
    })
  }

  /// Send a chat message with terminal context
  pub async fn send_message(
    &self,
    user_message: &str,
    context: &TerminalContext,
    conversation_history: &[ChatMessage],
  ) -> Result<String> {
    // Build the full prompt with context
    let context_str = prompts::format_context(
      &context.history_lines,
      &context.cwd,
      context.last_exit_code,
    );

    let full_message = format!("{}\n\n{}", context_str, user_message);

    // Build chat request
    let mut messages = Vec::new();

    // Add system prompt
    messages.push(ChatMessage::system(prompts::system_prompt()));

    // Add conversation history
    messages.extend_from_slice(conversation_history);

    // Add current message
    messages.push(ChatMessage::user(full_message));

    let chat_req = ChatRequest::new(messages);

    // Send request based on provider
    let response = match self.provider {
      Provider::Anthropic => self
        .client
        .exec_chat(&self.model, chat_req, None)
        .await
        .context("Failed to send message to Anthropic")?,
      Provider::OpenAI => self
        .client
        .exec_chat(&self.model, chat_req, None)
        .await
        .context("Failed to send message to OpenAI")?,
      Provider::Gemini => self
        .client
        .exec_chat(&self.model, chat_req, None)
        .await
        .context("Failed to send message to Gemini")?,
      Provider::Ollama => self
        .client
        .exec_chat(&self.model, chat_req, None)
        .await
        .context("Failed to send message to Ollama")?,
    };

    // Extract text from response
    let text = response
      .content_text_as_str()
      .context("No text in response")?
      .to_string();

    Ok(text)
  }

  /// Send a message and stream the response
  pub async fn send_message_stream(
    &self,
    user_message: &str,
    context: &TerminalContext,
    conversation_history: &[ChatMessage],
  ) -> Result<ChatStreamResponse> {
    // Build the full prompt with context
    let context_str = prompts::format_context(
      &context.history_lines,
      &context.cwd,
      context.last_exit_code,
    );

    let full_message = format!("{}\n\n{}", context_str, user_message);

    // Build chat request
    let mut messages = Vec::new();
    messages.push(ChatMessage::system(prompts::system_prompt()));
    messages.extend_from_slice(conversation_history);
    messages.push(ChatMessage::user(full_message));

    let chat_req = ChatRequest::new(messages);

    // Get streaming response
    let stream = self
      .client
      .exec_chat_stream(&self.model, chat_req, None)
      .await
      .context("Failed to create streaming chat")?;

    // Return the stream directly
    // Note: ChatStreamResponse implements its own streaming interface
    Ok(stream)
  }

  pub fn provider(&self) -> Provider {
    self.provider
  }

  pub fn model(&self) -> &str {
    &self.model
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_terminal_context_creation() {
    let ctx = TerminalContext::new(
      vec!["line1".to_string(), "line2".to_string()],
      PathBuf::from("/tmp"),
      Some(0),
    );

    assert_eq!(ctx.history_lines.len(), 2);
    assert_eq!(ctx.cwd, PathBuf::from("/tmp"));
    assert_eq!(ctx.last_exit_code, Some(0));
  }

  #[test]
  fn test_empty_context() {
    let ctx = TerminalContext::empty(PathBuf::from("/home"));
    assert!(ctx.history_lines.is_empty());
    assert!(ctx.last_exit_code.is_none());
  }
}
