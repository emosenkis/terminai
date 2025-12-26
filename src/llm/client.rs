// TERMIN.AI: LLM Client using Python/PydanticAI via PyO3
//
// This module provides the LLM client for Termin.AI using Python's PydanticAI
// library through PyO3 bindings.

use anyhow::{Context, Result};
use futures::stream::Stream;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use super::providers::Provider;
use super::{ChatMessage, TerminalContext};

/// Suggested command from the AI
#[derive(Debug, Clone)]
pub struct SuggestedCommand {
  pub command: String,
  pub explanation: String,
  pub raw: bool,
}

/// LLM client (Python-based via PyO3)
pub struct LLMClient {
  provider: Provider,
  model_name: String,
  /// Python client instance (held across GIL boundary)
  py_client: Py<PyAny>,
  /// Current working directory for tools
  cwd: Arc<Mutex<PathBuf>>,
  /// Scrollback buffer for tools
  scrollback_buffer: Arc<Mutex<Vec<String>>>,
}

impl LLMClient {
  /// Initialize the LLM client
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

    let client = Python::with_gil(|py| -> Result<Self> {
      Ok(Self {
        provider,
        model_name,
        py_client: py_client.clone_ref(py),
        cwd: Arc::new(Mutex::new(PathBuf::from("."))),
        scrollback_buffer: Arc::new(Mutex::new(Vec::new())),
      })
    })?;

    Ok(client)
  }

  /// Create new client with custom endpoint (reserved for future use)
  pub async fn new_with_endpoint(
    provider: Provider,
    model: Option<String>,
    _endpoint: Option<String>,
  ) -> Result<Self> {
    // Custom endpoints not yet supported in Python backend
    // For now, just create a normal client
    Self::new(provider, model).await
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
      "Streaming not yet implemented. Use send_message() instead."
    ))
  }

  /// Send a non-streaming message
  ///
  /// Note: Currently collects the entire response before returning.
  /// Streaming implementation is pending due to complexity of async bridge.
  pub async fn send_message(
    &self,
    user_message: &str,
    context: &TerminalContext,
    conversation_history: &[ChatMessage],
  ) -> Result<String> {
    // Use blocking approach within with_gil
    // TODO: Use pyo3_async_runtimes::tokio::into_future for true async

    let user_message = user_message.to_string();
    let context = context.clone();
    let conversation_history = conversation_history.to_vec();

    // Clone py_client with GIL
    let py_client = Python::with_gil(|py| self.py_client.clone_ref(py));

    let result = tokio::task::spawn_blocking(move || {
      Python::with_gil(|py| -> Result<String> {
        // Build context dict
        let context_dict = PyDict::new_bound(py);
        context_dict
          .set_item("cwd", context.cwd.to_string_lossy().to_string())
          .map_err(|e| anyhow::anyhow!("Failed to set cwd: {}", e))?;

        let history = PyList::empty_bound(py);
        for line in &context.history_lines {
          history
            .append(line)
            .map_err(|e| anyhow::anyhow!("Failed to append history: {}", e))?;
        }
        context_dict
          .set_item("history_lines", history)
          .map_err(|e| anyhow::anyhow!("Failed to set history_lines: {}", e))?;

        if let Some(code) = context.last_exit_code {
          context_dict
            .set_item("last_exit_code", code)
            .map_err(|e| anyhow::anyhow!("Failed to set exit code: {}", e))?;
        } else {
          context_dict
            .set_item("last_exit_code", py.None())
            .map_err(|e| anyhow::anyhow!("Failed to set exit code: {}", e))?;
        }

        // Build history list
        let history_list = PyList::empty_bound(py);
        for msg in &conversation_history {
          let dict = PyDict::new_bound(py);
          dict
            .set_item("role", &msg.role)
            .map_err(|e| anyhow::anyhow!("Failed to set role: {}", e))?;
          dict
            .set_item("content", &msg.content)
            .map_err(|e| anyhow::anyhow!("Failed to set content: {}", e))?;
          history_list
            .append(dict)
            .map_err(|e| anyhow::anyhow!("Failed to append message: {}", e))?;
        }

        // Call Python send_message_stream and collect all chunks
        // Note: This is not truly async - we block until all chunks are received
        // For true async streaming, we need pyo3-async-runtimes
        let py_result = py_client
          .call_method1(
            py,
            "send_message_stream",
            (user_message.clone(), context_dict, history_list),
          )
          .map_err(|e| {
            anyhow::anyhow!("Failed to call send_message_stream: {}", e)
          })?;

        // Import asyncio to run the async generator
        let asyncio = py
          .import("asyncio")
          .map_err(|e| anyhow::anyhow!("Failed to import asyncio: {}", e))?;

        // Collect all chunks from the async iterator
        let collect_code = r#"
async def collect_stream(stream):
    chunks = []
    async for chunk in stream:
        chunks.append(chunk)
    return "".join(chunks)
"#;

        // Execute the collection function
        let locals = PyDict::new_bound(py);
        py.run_bound(collect_code, None, Some(&locals))
          .map_err(|e| {
            anyhow::anyhow!("Failed to define collect function: {}", e)
          })?;

        let collect_fn = locals
          .get_item("collect_stream")
          .map_err(|e| anyhow::anyhow!("Failed to get collect_stream: {}", e))?
          .context("collect_stream not found")?;

        // Run the coroutine and get result
        let coroutine = collect_fn.call1((py_result,)).map_err(|e| {
          anyhow::anyhow!("Failed to call collect_stream: {}", e)
        })?;

        let result = asyncio
          .call_method1("run", (coroutine,))
          .map_err(|e| anyhow::anyhow!("Failed to run coroutine: {}", e))?;

        let response = result
          .extract::<String>()
          .map_err(|e| anyhow::anyhow!("Failed to extract response: {}", e))?;

        Ok(response)
      })
    })
    .await
    .map_err(|e| anyhow::anyhow!("Task join error: {}", e))??;

    Ok(result)
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
#[path = "client_test.rs"]
mod client_test;
