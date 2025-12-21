use anyhow::{Context, Result};
use futures::stream::{Stream, StreamExt};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex, RwLock};

use super::prompts;
use super::providers::Provider;
use super::tools::{
  GrepFilesTool, ReadFileTool, ReadScrollbackTool, SuggestCommandTool,
  SuggestedCommand,
};

use rig::agent::MultiTurnStreamItem;
use rig::client::CompletionClient;
use rig::completion::Prompt;
use rig::streaming::{StreamedAssistantContent, StreamingPrompt};

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

/// Simple message structure for conversation history
#[derive(Debug, Clone)]
pub struct ChatMessage {
  pub role: String,
  pub content: String,
}

impl ChatMessage {
  pub fn system(content: impl Into<String>) -> Self {
    Self {
      role: "system".to_string(),
      content: content.into(),
    }
  }

  pub fn user(content: impl Into<String>) -> Self {
    Self {
      role: "user".to_string(),
      content: content.into(),
    }
  }

  pub fn assistant(content: impl Into<String>) -> Self {
    Self {
      role: "assistant".to_string(),
      content: content.into(),
    }
  }
}

/// LLM client for interacting with various AI providers
pub struct LLMClient {
  provider: Provider,
  model_name: String,
  custom_endpoint: Option<String>,
  // Tool state
  cwd: Arc<RwLock<PathBuf>>,
  suggested_commands: Arc<Mutex<Vec<SuggestedCommand>>>,
  scrollback_buffer: Arc<RwLock<Vec<String>>>,
}

impl LLMClient {
  /// Create a new LLM client
  pub async fn new(provider: Provider, model: Option<String>) -> Result<Self> {
    Self::new_with_endpoint(provider, model, None).await
  }

  /// Create a new LLM client with custom endpoint
  pub async fn new_with_endpoint(
    provider: Provider,
    model: Option<String>,
    custom_endpoint: Option<String>,
  ) -> Result<Self> {
    let model_name =
      model.unwrap_or_else(|| provider.default_model().to_string());

    Ok(Self {
      provider,
      model_name,
      custom_endpoint,
      cwd: Arc::new(RwLock::new(PathBuf::from("."))),
      suggested_commands: Arc::new(Mutex::new(Vec::new())),
      scrollback_buffer: Arc::new(RwLock::new(Vec::new())),
    })
  }

  /// Set the current working directory for file operations
  pub fn set_cwd(&self, cwd: PathBuf) -> Result<()> {
    *self
      .cwd
      .write()
      .map_err(|_| anyhow::anyhow!("Failed to acquire cwd lock"))? = cwd;
    Ok(())
  }

  /// Update the scrollback buffer
  pub fn update_scrollback(&self, lines: Vec<String>) -> Result<()> {
    *self
      .scrollback_buffer
      .write()
      .map_err(|_| anyhow::anyhow!("Failed to acquire scrollback lock"))? =
      lines;
    Ok(())
  }

  /// Get and clear suggested commands
  pub fn take_suggested_commands(&self) -> Result<Vec<SuggestedCommand>> {
    let mut commands = self
      .suggested_commands
      .lock()
      .map_err(|_| anyhow::anyhow!("Failed to acquire commands lock"))?;
    Ok(std::mem::take(&mut *commands))
  }

  /// Build the full message with context
  fn build_full_message(
    &self,
    user_message: &str,
    context: &TerminalContext,
  ) -> String {
    let context_str = prompts::format_context(
      &context.history_lines,
      &context.cwd,
      context.last_exit_code,
    );
    format!("{}\n\n{}", context_str, user_message)
  }

  /// Build preamble with conversation history
  fn build_preamble(&self, conversation_history: &[ChatMessage]) -> String {
    let mut preamble_parts = vec![prompts::system_prompt().to_string()];

    for msg in conversation_history {
      match msg.role.as_str() {
        "user" => preamble_parts.push(format!("User: {}", msg.content)),
        "assistant" => {
          preamble_parts.push(format!("Assistant: {}", msg.content))
        }
        _ => {}
      }
    }

    preamble_parts.join("\n\n")
  }

  /// Send a chat message with terminal context (non-streaming)
  pub async fn send_message(
    &self,
    user_message: &str,
    context: &TerminalContext,
    conversation_history: &[ChatMessage],
  ) -> Result<String> {
    let full_message = self.build_full_message(user_message, context);
    let preamble = self.build_preamble(conversation_history);

    // Macro to build agent with tools for each provider
    macro_rules! build_agent_with_tools {
      ($client:expr) => {{
        $client
          .agent(&self.model_name)
          .preamble(&preamble)
          .max_tokens(4096)
          .tool(ReadFileTool::new(Arc::clone(&self.cwd)))
          .tool(SuggestCommandTool::new(Arc::clone(
            &self.suggested_commands,
          )))
          .tool(ReadScrollbackTool::new(Arc::clone(&self.scrollback_buffer)))
          .tool(GrepFilesTool::new(Arc::clone(&self.cwd)))
          .build()
      }};
    }

    let response = match self.provider {
      Provider::Anthropic => {
        use rig::providers::anthropic;
        let api_key = std::env::var("ANTHROPIC_API_KEY")
          .context("ANTHROPIC_API_KEY environment variable not set")?;
        let client: anthropic::Client = anthropic::Client::new(&api_key)?;
        let agent = build_agent_with_tools!(client);
        agent
          .prompt(&full_message)
          .await
          .context("Failed to send message to Anthropic")?
      }
      Provider::OpenAI => {
        use rig::providers::openai;
        let api_key = std::env::var("OPENAI_API_KEY")
          .context("OPENAI_API_KEY environment variable not set")?;
        let client: openai::Client = openai::Client::new(&api_key)?;
        let agent = build_agent_with_tools!(client);
        agent
          .prompt(&full_message)
          .await
          .context("Failed to send message to OpenAI")?
      }
      Provider::Gemini => {
        use rig::providers::gemini;
        let api_key = std::env::var("GOOGLE_API_KEY")
          .context("GOOGLE_API_KEY environment variable not set")?;
        let client: gemini::Client = gemini::Client::new(&api_key)?;
        let agent = build_agent_with_tools!(client);
        agent
          .prompt(&full_message)
          .await
          .context("Failed to send message to Gemini")?
      }
      Provider::Ollama => {
        use rig::client::Nothing;
        use rig::providers::ollama;
        let endpoint = self
          .custom_endpoint
          .as_deref()
          .unwrap_or("http://localhost:11434");
        let client: ollama::Client = ollama::Client::builder()
          .api_key(Nothing)
          .base_url(endpoint)
          .build()?;
        let agent = build_agent_with_tools!(client);
        agent
          .prompt(&full_message)
          .await
          .context("Failed to send message to Ollama")?
      }
      Provider::OpenRouter => {
        use rig::providers::openrouter;
        let api_key = std::env::var("OPENROUTER_API_KEY")
          .context("OPENROUTER_API_KEY environment variable not set")?;
        let client: openrouter::Client = openrouter::Client::new(&api_key)?;
        let agent = build_agent_with_tools!(client);
        agent
          .prompt(&full_message)
          .await
          .context("Failed to send message to OpenRouter")?
      }
    };

    Ok(response)
  }

  /// Send a message and stream the response
  pub async fn send_message_stream(
    &self,
    user_message: &str,
    context: &TerminalContext,
    conversation_history: &[ChatMessage],
  ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
    let full_message = self.build_full_message(user_message, context);
    let preamble = self.build_preamble(conversation_history);

    // Macro to build streaming agent with tools
    macro_rules! build_streaming_agent {
      ($client:expr) => {{
        let agent = $client
          .agent(&self.model_name)
          .preamble(&preamble)
          .max_tokens(4096)
          .tool(ReadFileTool::new(Arc::clone(&self.cwd)))
          .tool(SuggestCommandTool::new(Arc::clone(
            &self.suggested_commands,
          )))
          .tool(ReadScrollbackTool::new(Arc::clone(&self.scrollback_buffer)))
          .tool(GrepFilesTool::new(Arc::clone(&self.cwd)))
          .build();
        let stream = agent.stream_prompt(&full_message).await;
        Box::pin(stream.map(|result| {
          result
            .map_err(|e| anyhow::Error::from(e))
            .and_then(|item| match item {
              MultiTurnStreamItem::StreamAssistantItem(
                StreamedAssistantContent::Text(text),
              ) => Ok(text.text),
              _ => Ok(String::new()),
            })
        })) as Pin<Box<dyn Stream<Item = Result<String>> + Send>>
      }};
    }

    let text_stream = match self.provider {
      Provider::Anthropic => {
        use rig::providers::anthropic;
        let api_key = std::env::var("ANTHROPIC_API_KEY")
          .context("ANTHROPIC_API_KEY environment variable not set")?;
        let client: anthropic::Client = anthropic::Client::new(&api_key)?;
        build_streaming_agent!(client)
      }
      Provider::OpenAI => {
        use rig::providers::openai;
        let api_key = std::env::var("OPENAI_API_KEY")
          .context("OPENAI_API_KEY environment variable not set")?;
        let client: openai::Client = openai::Client::new(&api_key)?;
        build_streaming_agent!(client)
      }
      Provider::Gemini => {
        use rig::providers::gemini;
        let api_key = std::env::var("GOOGLE_API_KEY")
          .context("GOOGLE_API_KEY environment variable not set")?;
        let client: gemini::Client = gemini::Client::new(&api_key)?;
        build_streaming_agent!(client)
      }
      Provider::Ollama => {
        use rig::client::Nothing;
        use rig::providers::ollama;
        let endpoint = self
          .custom_endpoint
          .as_deref()
          .unwrap_or("http://localhost:11434");
        let client: ollama::Client = ollama::Client::builder()
          .api_key(Nothing)
          .base_url(endpoint)
          .build()?;
        build_streaming_agent!(client)
      }
      Provider::OpenRouter => {
        use rig::providers::openrouter;
        let api_key = std::env::var("OPENROUTER_API_KEY")
          .context("OPENROUTER_API_KEY environment variable not set")?;
        let client: openrouter::Client = openrouter::Client::new(&api_key)?;
        build_streaming_agent!(client)
      }
    };

    Ok(text_stream)
  }

  pub fn provider(&self) -> Provider {
    self.provider
  }

  pub fn model(&self) -> &str {
    &self.model_name
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

  #[test]
  fn test_chat_message_creation() {
    let msg = ChatMessage::user("Hello");
    assert_eq!(msg.role, "user");
    assert_eq!(msg.content, "Hello");

    let msg = ChatMessage::assistant("Hi there");
    assert_eq!(msg.role, "assistant");
    assert_eq!(msg.content, "Hi there");

    let msg = ChatMessage::system("You are a helpful assistant");
    assert_eq!(msg.role, "system");
    assert_eq!(msg.content, "You are a helpful assistant");
  }
}
