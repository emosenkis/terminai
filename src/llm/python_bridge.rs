///! Python-Rust bridge for LLM streaming
///!
///! This module handles the complex async bridging between Python's asyncio
///! and Rust's tokio runtime using pyo3-async-runtimes.
use anyhow::{Context, Result};
use futures::Stream;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use pyo3_async_runtimes::TaskLocals;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

/// Python event loop runner that keeps the event loop alive in a background thread
pub struct EventLoopRunner {
  _handle: std::thread::JoinHandle<()>,
  event_loop: Py<PyAny>,
}

impl EventLoopRunner {
  /// Create and start a new event loop runner in a background thread
  pub fn new() -> Result<Self> {
    let (tx, rx) = std::sync::mpsc::channel::<Py<PyAny>>();

    let handle = std::thread::spawn(move || {
      Python::with_gil(|py| {
        // Create new event loop
        let asyncio = py.import("asyncio").expect("Failed to import asyncio");
        let event_loop = asyncio
          .call_method0("new_event_loop")
          .expect("Failed to create event loop");

        // Set as the event loop for this thread
        asyncio
          .call_method1("set_event_loop", (&event_loop,))
          .expect("Failed to set event loop");

        // Send a copy of the event loop back to the main thread
        tx.send(event_loop.clone().unbind())
          .expect("Failed to send event loop");

        // Run the event loop forever
        log::info!(
          "[Python EventLoop] Starting event loop in background thread"
        );
        if let Err(e) = event_loop.call_method0("run_forever") {
          log::error!("[Python EventLoop] Event loop error: {}", e);
        }
        log::info!("[Python EventLoop] Event loop stopped");
      });
    });

    // Wait for event loop to be created
    let event_loop = rx
      .recv()
      .context("Failed to receive event loop from background thread")?;

    log::info!("[Python EventLoop] Background event loop created and running");

    Ok(Self {
      _handle: handle,
      event_loop,
    })
  }

  /// Get a reference to the running event loop
  pub fn event_loop(&self) -> &Py<PyAny> {
    &self.event_loop
  }

  /// Create TaskLocals from this event loop
  pub fn task_locals(&self, py: Python<'_>) -> TaskLocals {
    TaskLocals::new(self.event_loop.bind(py).clone())
  }
}

impl Drop for EventLoopRunner {
  fn drop(&mut self) {
    // Stop the event loop when dropped
    Python::with_gil(|py| {
      // Get the event loop's stop method and call it via call_soon_threadsafe
      if let Err(e) = self.event_loop.bind(py).call_method1(
        "call_soon_threadsafe",
        (self.event_loop.bind(py).getattr("stop").ok(),),
      ) {
        log::warn!("[Python EventLoop] Failed to stop event loop: {}", e);
      }
    });
  }
}

/// Convert a Python async iterator of strings to a Rust Stream
pub fn python_stream_to_rust(
  event_loop_runner: Arc<EventLoopRunner>,
  py_async_iter: Py<PyAny>,
) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
  use futures::StreamExt;

  log::debug!(
    "[Python Bridge] Converting Python async iterator to Rust stream"
  );

  // Create TaskLocals with the running event loop
  let rust_stream = Python::with_gil(|py| {
    let locals = event_loop_runner.task_locals(py);

    log::debug!("[Python Bridge] Created TaskLocals from running event loop");

    pyo3_async_runtimes::tokio::into_stream_with_locals_v2(
      locals,
      py_async_iter.into_bound(py),
    )
    .context("Failed to convert async iterator to stream")
  })?;

  log::info!("[Python Bridge] Successfully converted to Rust stream");

  let mut chunk_count = 0usize;

  // Map stream to extract String chunks
  let string_stream = rust_stream.map(move |py_obj| {
    chunk_count += 1;
    if chunk_count <= 5 || chunk_count % 10 == 0 {
      log::debug!("[Python Bridge] Processing chunk #{}", chunk_count);
    }

    // Extract string from Python object
    Python::with_gil(|py| {
      py_obj
        .extract::<String>(py)
        .context("Failed to extract string from Python object")
    })
  });

  Ok(Box::pin(string_stream))
}

/// Setup Python-to-Rust logging bridge
#[pyfunction]
fn rust_log_callback(level: String, message: String) {
  match level.as_str() {
    "debug" => log::debug!("[Python] {}", message),
    "info" => log::info!("[Python] {}", message),
    "warn" => log::warn!("[Python] {}", message),
    "error" => log::error!("[Python] {}", message),
    _ => log::info!("[Python] {}", message),
  }
}

/// Initialize Python logging bridge
pub fn setup_python_logging(py: Python<'_>) -> PyResult<()> {
  let module = py.import("terminai_llm")?;
  let logging_callback = wrap_pyfunction!(rust_log_callback, py)?;
  module
    .getattr("setup_rust_logging")?
    .call1((logging_callback,))?;
  log::info!("[Python Bridge] Logging bridge initialized");
  Ok(())
}

/// Helper to build Python context dict from Rust data
pub fn build_context_dict<'py>(
  py: Python<'py>,
  cwd: &str,
  history_lines: &[String],
  last_exit_code: Option<i32>,
) -> PyResult<Bound<'py, PyDict>> {
  let context_dict = PyDict::new(py);
  context_dict.set_item("cwd", cwd)?;

  let history = PyList::empty(py);
  for line in history_lines {
    history.append(line)?;
  }
  context_dict.set_item("history_lines", history)?;

  if let Some(code) = last_exit_code {
    context_dict.set_item("last_exit_code", code)?;
  } else {
    context_dict.set_item("last_exit_code", py.None())?;
  }

  Ok(context_dict)
}

/// Helper to build conversation history list
pub fn build_history_list<'py>(
  py: Python<'py>,
  history: &[(String, String)], // (role, content) pairs
) -> PyResult<Bound<'py, PyList>> {
  let history_list = PyList::empty(py);
  for (role, content) in history {
    let dict = PyDict::new(py);
    dict.set_item("role", role)?;
    dict.set_item("content", content)?;
    history_list.append(dict)?;
  }
  Ok(history_list)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_event_loop_runner_creation() {
    pyo3::prepare_freethreaded_python();

    // Create event loop runner
    let runner =
      EventLoopRunner::new().expect("Failed to create event loop runner");

    // Verify we can access the event loop
    Python::with_gil(|py| {
      let loop_ref = runner.event_loop().bind(py);
      assert!(loop_ref.hasattr("call_soon_threadsafe").unwrap());
    });

    // Drop will stop the event loop
    drop(runner);
  }

  #[tokio::test]
  async fn test_python_stream_basic() {
    use std::ffi::CString;

    pyo3::prepare_freethreaded_python();

    // Initialize pyo3-async-runtimes
    let pyo3_rt = tokio::runtime::Runtime::new().unwrap();
    let pyo3_rt: &'static tokio::runtime::Runtime =
      Box::leak(Box::new(pyo3_rt));
    pyo3_async_runtimes::tokio::init_with_runtime(pyo3_rt).ok();

    // Create event loop runner
    let runner =
      Arc::new(EventLoopRunner::new().expect("Failed to create runner"));

    // Create a simple Python async generator
    let py_async_iter = Python::with_gil(|py| -> PyResult<Py<PyAny>> {
      let code = r#"
async def test_gen():
    for i in range(5):
        yield str(i)
"#;
      let module = PyModule::from_code(
        py,
        &CString::new(code).unwrap(),
        &CString::new("test.py").unwrap(),
        &CString::new("test").unwrap(),
      )?;
      let gen_func = module.getattr("test_gen")?;
      Ok(gen_func.call0()?.unbind())
    })
    .expect("Failed to create test generator");

    // Convert to Rust stream
    let mut stream = python_stream_to_rust(runner.clone(), py_async_iter)
      .expect("Failed to create stream");

    // Collect results
    use futures::StreamExt;
    let results: Vec<String> =
      stream.filter_map(|r| async move { r.ok() }).collect().await;

    assert_eq!(results, vec!["0", "1", "2", "3", "4"]);
  }
}
