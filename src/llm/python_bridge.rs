// TERMIN.AI: Python LLM Bridge using PyO3
//
// This module provides a bridge between Rust and Python for LLM interactions.
// It wraps the Python LLMClient implemented with PydanticAI.

use anyhow::{Context, Result};
use futures::stream::{Stream, StreamExt};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use super::providers::Provider;
use super::tools::SuggestedCommand;
use super::{ChatMessage, TerminalContext};

/// Python LLM client bridge
pub struct PythonLLMBridge {
  provider: Provider,
  model_name: String,
  /// Python client instance (held across GIL boundary)
  py_client: Py<PyAny>,
  /// Current working directory for tools
  cwd: Arc<Mutex<PathBuf>>,
  /// Scrollback buffer for tools
  scrollback_buffer: Arc<Mutex<Vec<String>>>,
}

impl PythonLLMBridge {
  /// Initialize the Python bridge
  pub async fn new(provider: Provider, model: Option<String>) -> Result<Self> {
    Self::new_with_api_key(provider, model, None).await
  }

  /// Initialize with explicit API key
  pub async fn new_with_api_key(
    provider: Provider,
    model: Option<String>,
    api_key: Option<String>,
  ) -> Result<Self> {
    let model_name = model
      .as_ref()
      .map(|s| s.clone())
      .unwrap_or_else(|| provider.default_model().to_string());
    let provider_str = provider.to_python_string();

    let py_client = Python::with_gil(|py| -> PyResult<Py<PyAny>> {
      // Add python directory to sys.path
      let sys = py.import("sys")?;
      let path = sys.getattr("path")?;
      path.call_method1("insert", (0, "../python"))?;

      // Import the LLM client module
      let module = py.import("terminai_llm")?;

      // Create client instance
      let kwargs = PyDict::new(py);
      kwargs.set_item("provider", provider_str)?;
      if let Some(ref m) = model {
        kwargs.set_item("model", m)?;
      }
      if let Some(ref key) = api_key {
        kwargs.set_item("api_key", key)?;
      }

      let client = module
        .getattr("LLMClient")?
        .call((), Some(&kwargs))?
        .into_py(py);

      Ok(client)
    })
    .context("Failed to initialize Python LLM client")?;

    Ok(Self {
      provider,
      model_name,
      py_client,
      cwd: Arc::new(Mutex::new(PathBuf::from("."))),
      scrollback_buffer: Arc::new(Mutex::new(Vec::new())),
    })
  }

  /// Set the current working directory
  pub fn set_cwd(&self, cwd: PathBuf) -> Result<()> {
    *self
      .cwd
      .lock()
      .map_err(|_| anyhow::anyhow!("Failed to acquire cwd lock"))? = cwd;
    Ok(())
  }

  /// Update the scrollback buffer
  pub fn update_scrollback(&self, lines: Vec<String>) -> Result<()> {
    *self
      .scrollback_buffer
      .lock()
      .map_err(|_| anyhow::anyhow!("Failed to acquire scrollback lock"))? =
      lines;
    Ok(())
  }

  /// Get and clear suggested commands from Python client
  pub fn take_suggested_commands(&self) -> Result<Vec<SuggestedCommand>> {
    Python::with_gil(|py| -> Result<Vec<SuggestedCommand>> {
      let commands_list = self
        .py_client
        .call_method0(py, "take_suggested_commands")
        .map_err(|e| {
          anyhow::anyhow!("Failed to call take_suggested_commands: {}", e)
        })?;

      let commands: Vec<_> =
        commands_list.extract::<Vec<Py<PyAny>>>(py).map_err(|e| {
          anyhow::anyhow!("Failed to extract commands list: {}", e)
        })?;

      let mut result = Vec::new();
      for cmd_dict in commands {
        let dict = cmd_dict.downcast_bound::<PyDict>(py).map_err(|e| {
          anyhow::anyhow!("Failed to downcast to PyDict: {}", e)
        })?;

        let command = dict
          .get_item("command")
          .map_err(|e| anyhow::anyhow!("Failed to get command field: {}", e))?
          .context("Missing command field")?
          .extract::<String>()
          .map_err(|e| {
            anyhow::anyhow!("Failed to extract command string: {}", e)
          })?;

        let explanation = dict
          .get_item("explanation")
          .map_err(|e| {
            anyhow::anyhow!("Failed to get explanation field: {}", e)
          })?
          .context("Missing explanation field")?
          .extract::<String>()
          .map_err(|e| {
            anyhow::anyhow!("Failed to extract explanation string: {}", e)
          })?;

        let raw = dict
          .get_item("raw")
          .map_err(|e| anyhow::anyhow!("Failed to get raw field: {}", e))?
          .context("Missing raw field")?
          .extract::<bool>()
          .map_err(|e| anyhow::anyhow!("Failed to extract raw bool: {}", e))?;

        result.push(SuggestedCommand {
          command,
          explanation,
          raw,
        });
      }

      Ok(result)
    })
  }

  /// Build context dict for Python
  fn build_context_dict(
    &self,
    py: Python,
    context: &TerminalContext,
  ) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("cwd", context.cwd.to_string_lossy().to_string())?;

    let history = PyList::empty(py);
    for line in &context.history_lines {
      history.append(line)?;
    }
    dict.set_item("history_lines", history)?;

    if let Some(code) = context.last_exit_code {
      dict.set_item("last_exit_code", code)?;
    } else {
      dict.set_item("last_exit_code", py.None())?;
    }

    Ok(dict.into())
  }

  /// Build history list for Python
  fn build_history_list(
    &self,
    py: Python,
    history: &[ChatMessage],
  ) -> PyResult<Py<PyList>> {
    let list = PyList::empty(py);
    for msg in history {
      let dict = PyDict::new(py);
      dict.set_item("role", &msg.role)?;
      dict.set_item("content", &msg.content)?;
      list.append(dict)?;
    }
    Ok(list.into())
  }

  /// Send a message and stream the response
  ///
  /// TODO: Implement proper async streaming from Python to Rust
  /// This is complex because it requires bridging Python's asyncio with Rust's async/await
  pub async fn send_message_stream(
    &self,
    _user_message: &str,
    _context: &TerminalContext,
    _conversation_history: &[ChatMessage],
  ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
    // TODO: Implement Python async iterator -> Rust stream conversion
    // This requires using pyo3-async-runtimes to properly bridge the async boundaries
    Err(anyhow::anyhow!(
      "Streaming not yet implemented in Python bridge. Use send_message() instead."
    ))
  }

  /// Send a non-streaming message
  ///
  /// TODO: This is a placeholder implementation. We need to properly implement
  /// async Python calls using pyo3-async-runtimes.
  pub async fn send_message(
    &self,
    _user_message: &str,
    _context: &TerminalContext,
    _conversation_history: &[ChatMessage],
  ) -> Result<String> {
    // TODO: Implement non-streaming message sending
    // This requires:
    // 1. Converting Rust types to Python types
    // 2. Calling the Python async method
    // 3. Awaiting the result properly across the language boundary
    Err(anyhow::anyhow!(
      "Non-streaming messages not yet implemented in Python bridge"
    ))
  }

  pub fn provider(&self) -> Provider {
    self.provider
  }

  pub fn model(&self) -> &str {
    &self.model_name
  }
}

// Helper trait for Provider to convert to Python string
trait ProviderExt {
  fn to_python_string(&self) -> &str;
}

impl ProviderExt for Provider {
  fn to_python_string(&self) -> &str {
    match self {
      Provider::Anthropic => "anthropic",
      Provider::OpenAI => "openai",
      Provider::Gemini => "google-vertex",
      Provider::Ollama => "ollama",
      Provider::OpenRouter => "openrouter",
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn test_bridge_creation() {
    // This test requires Python environment to be set up
    // Skip if ANTHROPIC_API_KEY is not set
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
      eprintln!("Skipping test: ANTHROPIC_API_KEY not set");
      return;
    }

    let bridge = PythonLLMBridge::new(Provider::Anthropic, None).await;
    assert!(
      bridge.is_ok(),
      "Failed to create bridge: {:?}",
      bridge.err()
    );
  }

  #[test]
  fn test_terminal_context_serialization() {
    Python::with_gil(|py| {
      let ctx = TerminalContext::new(
        vec!["line1".to_string(), "line2".to_string()],
        PathBuf::from("/tmp"),
        Some(0),
      );

      // This would be used internally
      let dict = PyDict::new(py);
      dict.set_item("cwd", "/tmp").unwrap();
      assert!(dict.contains("cwd").unwrap());
    });
  }
}
