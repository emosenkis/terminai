use anyhow::{Context, Result};
use genai::Client;
use genai::chat::{ChatMessage, ChatRequest};
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
  custom_endpoint: Option<String>,
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
    let mut model =
      model.unwrap_or_else(|| provider.default_model().to_string());

    // For OpenRouter, we need to prefix the model with the adapter type
    // to ensure genai uses the OpenAI adapter instead of incorrectly
    // detecting it as Ollama (due to the "/" in model names like "anthropic/claude")
    if custom_endpoint.is_some() && provider == Provider::OpenRouter {
      // Only add prefix if not already present
      if !model.starts_with("openai::") {
        model = format!("openai::{}", model);
      }
    }

    // Initialize genai client with custom endpoint if needed
    let client = if let Some(ref endpoint) = custom_endpoint {
      // For OpenRouter or custom endpoints, use a ServiceTargetResolver
      use genai::adapter::AdapterKind;
      use genai::resolver::{AuthData, Endpoint, ServiceTargetResolver};
      use genai::{ModelIden, ServiceTarget};

      let endpoint_url = endpoint.clone();
      let provider_copy = provider;

      let target_resolver = ServiceTargetResolver::from_resolver_fn(
        move |service_target: ServiceTarget| -> Result<ServiceTarget, genai::resolver::Error> {
          let ServiceTarget { model, auth: original_auth, .. } = service_target;
          let endpoint = Endpoint::from_owned(endpoint_url.clone());

          // For OpenRouter, use OpenAI adapter kind but OPENROUTER_API_KEY
          let (adapter_kind, auth) = match provider_copy {
            Provider::OpenRouter => (
              AdapterKind::OpenAI,
              AuthData::from_env("OPENROUTER_API_KEY"),
            ),
            Provider::Anthropic => (
              AdapterKind::Anthropic,
              AuthData::from_env("ANTHROPIC_API_KEY"),
            ),
            Provider::OpenAI => (
              AdapterKind::OpenAI,
              AuthData::from_env("OPENAI_API_KEY"),
            ),
            Provider::Gemini => (
              AdapterKind::Gemini,
              AuthData::from_env("GOOGLE_API_KEY"),
            ),
            Provider::Ollama => {
              // Ollama doesn't need an API key
              (AdapterKind::Ollama, original_auth)
            }
          };

          let model = ModelIden::new(adapter_kind, model.model_name);
          Ok(ServiceTarget { endpoint, auth, model })
        },
      );

      let mut builder =
        Client::builder().with_service_target_resolver(target_resolver);

      // OpenRouter requires specific headers
      if provider == Provider::OpenRouter {
        use genai::WebConfig;
        use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

        let mut headers = HeaderMap::new();
        // OpenRouter requires HTTP-Referer header
        headers.insert(
          HeaderName::from_static("http-referer"),
          HeaderValue::from_static("https://github.com/yourusername/terminai"),
        );
        // Optional: X-Title for display in OpenRouter dashboard
        headers.insert(
          HeaderName::from_static("x-title"),
          HeaderValue::from_static("Termin.AI"),
        );

        let web_config = WebConfig::default().with_default_headers(headers);
        builder = builder.with_web_config(web_config);
      }

      builder.build()
    } else {
      Client::default()
    };

    Ok(Self {
      client,
      provider,
      model,
      custom_endpoint,
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
      Provider::OpenRouter => self
        .client
        .exec_chat(&self.model, chat_req, None)
        .await
        .context("Failed to send message to OpenRouter")?,
    };

    // Extract text from response
    let text = response
      .first_text()
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
  ) -> Result<impl futures::Stream<Item = Result<String>>> {
    use futures::stream::StreamExt;

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

    // Send streaming request based on provider
    let stream_response = match self.provider {
      Provider::Anthropic => self
        .client
        .exec_chat_stream(&self.model, chat_req, None)
        .await
        .context("Failed to stream message from Anthropic")?,
      Provider::OpenAI => self
        .client
        .exec_chat_stream(&self.model, chat_req, None)
        .await
        .context("Failed to stream message from OpenAI")?,
      Provider::Gemini => self
        .client
        .exec_chat_stream(&self.model, chat_req, None)
        .await
        .context("Failed to stream message from Gemini")?,
      Provider::Ollama => self
        .client
        .exec_chat_stream(&self.model, chat_req, None)
        .await
        .context("Failed to stream message from Ollama")?,
      Provider::OpenRouter => self
        .client
        .exec_chat_stream(&self.model, chat_req, None)
        .await
        .context("Failed to stream message from OpenRouter")?,
    };

    // Convert the ChatStream to a stream of strings
    use genai::chat::ChatStreamEvent;

    Ok(stream_response.stream.map(|result| {
      result
        .map(|event| match event {
          ChatStreamEvent::Chunk(chunk) => chunk.content,
          ChatStreamEvent::ReasoningChunk(chunk) => chunk.content,
          ChatStreamEvent::ToolCallChunk(_) => String::new(), // Ignore tool call chunks for now
          ChatStreamEvent::Start => String::new(),
          ChatStreamEvent::End(_) => String::new(),
        })
        .map_err(|e| anyhow::Error::from(e))
    }))
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
