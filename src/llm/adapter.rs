// TERMIN.AI: LLM Client Adapter
//
// This adapter provides a unified interface that can use either:
// - Rig-based LLM client (default)
// - Python-based LLM client (when python-llm feature is enabled)

use anyhow::Result;
use futures::stream::Stream;
use std::path::PathBuf;
use std::pin::Pin;

use super::providers::Provider;
use super::tools::SuggestedCommand;
use super::{ChatMessage, TerminalContext};

#[cfg(feature = "python-llm")]
use super::python_bridge::PythonLLMBridge;

#[cfg(not(feature = "python-llm"))]
use super::client::LLMClient as RigLLMClient;

/// Unified LLM client that can use either Rig or Python backend
pub enum LLMClientAdapter {
  #[cfg(not(feature = "python-llm"))]
  Rig(RigLLMClient),

  #[cfg(feature = "python-llm")]
  Python(PythonLLMBridge),
}

impl LLMClientAdapter {
  /// Create a new LLM client adapter
  pub async fn new(provider: Provider, model: Option<String>) -> Result<Self> {
    #[cfg(feature = "python-llm")]
    {
      Ok(Self::Python(
        PythonLLMBridge::new(provider, model).await?,
      ))
    }

    #[cfg(not(feature = "python-llm"))]
    {
      Ok(Self::Rig(RigLLMClient::new(provider, model).await?))
    }
  }

  /// Create a new LLM client with custom endpoint
  pub async fn new_with_endpoint(
    provider: Provider,
    model: Option<String>,
    endpoint: Option<String>,
  ) -> Result<Self> {
    #[cfg(feature = "python-llm")]
    {
      // TODO: Python bridge doesn't support custom endpoints yet
      let _ = endpoint;
      Ok(Self::Python(
        PythonLLMBridge::new(provider, model).await?,
      ))
    }

    #[cfg(not(feature = "python-llm"))]
    {
      Ok(Self::Rig(
        RigLLMClient::new_with_endpoint(provider, model, endpoint).await?,
      ))
    }
  }

  /// Set the current working directory
  pub fn set_cwd(&self, cwd: PathBuf) -> Result<()> {
    match self {
      #[cfg(feature = "python-llm")]
      Self::Python(client) => client.set_cwd(cwd),

      #[cfg(not(feature = "python-llm"))]
      Self::Rig(client) => client.set_cwd(cwd),
    }
  }

  /// Update the scrollback buffer
  pub fn update_scrollback(&self, lines: Vec<String>) -> Result<()> {
    match self {
      #[cfg(feature = "python-llm")]
      Self::Python(client) => client.update_scrollback(lines),

      #[cfg(not(feature = "python-llm"))]
      Self::Rig(client) => client.update_scrollback(lines),
    }
  }

  /// Get and clear suggested commands
  pub fn take_suggested_commands(&self) -> Result<Vec<SuggestedCommand>> {
    match self {
      #[cfg(feature = "python-llm")]
      Self::Python(client) => client.take_suggested_commands(),

      #[cfg(not(feature = "python-llm"))]
      Self::Rig(client) => client.take_suggested_commands(),
    }
  }

  /// Send a message and stream the response
  pub async fn send_message_stream(
    &self,
    user_message: &str,
    context: &TerminalContext,
    conversation_history: &[ChatMessage],
  ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
    match self {
      #[cfg(feature = "python-llm")]
      Self::Python(client) => {
        // Python bridge doesn't support streaming yet, fall back to non-streaming
        let response = client
          .send_message(user_message, context, conversation_history)
          .await?;

        // Create a stream that yields the full response at once
        use futures::stream;
        Ok(Box::pin(stream::once(async move { Ok(response) })))
      }

      #[cfg(not(feature = "python-llm"))]
      Self::Rig(client) => {
        client
          .send_message_stream(user_message, context, conversation_history)
          .await
      }
    }
  }

  /// Send a non-streaming message
  pub async fn send_message(
    &self,
    user_message: &str,
    context: &TerminalContext,
    conversation_history: &[ChatMessage],
  ) -> Result<String> {
    match self {
      #[cfg(feature = "python-llm")]
      Self::Python(client) => {
        client
          .send_message(user_message, context, conversation_history)
          .await
      }

      #[cfg(not(feature = "python-llm"))]
      Self::Rig(client) => {
        client
          .send_message(user_message, context, conversation_history)
          .await
      }
    }
  }
}

#[cfg(test)]
#[path = "adapter_test.rs"]
mod adapter_test;
