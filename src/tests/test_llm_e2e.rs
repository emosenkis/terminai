// End-to-end test for LLM integration
//
// This test exercises the full flow:
// 1. Rust spawns Python subprocess (real prod code)
// 2. Python subprocess uses mock LLM server (via OpenAI-compatible API)
// 3. Rust sends messages via AG-UI client (real prod code)
// 4. Responses flow back through the full stack
//
// No code duplication - uses same code as production!

use anyhow::Result;
use futures::StreamExt;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::sleep;

use termin::llm::AgUiClient;
use termin::llm_subprocess::LlmSubprocessConfig;

/// Helper to spawn the mock LLM server
struct MockLlmServer {
  child: tokio::process::Child,
  port: u16,
}

impl MockLlmServer {
  /// Spawn the mock LLM server on an available port
  async fn spawn() -> Result<Self> {
    // Find an available port
    let port = find_available_port().await?;

    // Get Python directory (CARGO_MANIFEST_DIR is .../termin.ai/src, we need .../termin.ai/python)
    let python_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .expect("Failed to get parent directory")
      .join("python");

    // Spawn uvicorn with the mock server
    log::debug!(
      "Spawning mock LLM server on port {} in {:?}",
      port,
      python_dir
    );
    let mut child = Command::new("uv")
      .arg("run")
      .arg("uvicorn")
      .arg("tests.mock_llm_server:create_mock_llm_app")
      .arg("--factory")
      .arg("--host")
      .arg("127.0.0.1")
      .arg("--port")
      .arg(port.to_string())
      .current_dir(&python_dir)
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .kill_on_drop(true)
      .spawn()
      .map_err(|e| anyhow::anyhow!("Failed to spawn mock LLM server: {}. Command: uv run uvicorn tests.mock_llm_server:create_mock_llm_app --factory --host 127.0.0.1 --port {} (cwd: {:?})", e, port, python_dir))?;

    // Monitor stderr for startup
    let stderr = child.stderr.take().expect("Failed to capture stderr");
    let mut stderr_reader = BufReader::new(stderr).lines();

    // Spawn task to log stderr
    tokio::spawn(async move {
      while let Ok(Some(line)) = stderr_reader.next_line().await {
        log::debug!("[Mock LLM stderr] {}", line);
      }
    });

    // Wait for server to be ready
    let base_url = format!("http://127.0.0.1:{}", port);
    for attempt in 1..=30 {
      sleep(Duration::from_millis(100)).await;

      match reqwest::get(format!("{}/health", base_url)).await {
        Ok(resp) if resp.status().is_success() => {
          log::info!("Mock LLM server ready on port {}", port);
          return Ok(Self { child, port });
        }
        _ => {
          if attempt == 30 {
            anyhow::bail!("Mock LLM server failed to start");
          }
        }
      }
    }

    unreachable!()
  }

  /// Get the base URL for the mock server
  fn base_url(&self) -> String {
    format!("http://127.0.0.1:{}", self.port)
  }

  /// Shutdown the server
  async fn shutdown(mut self) -> Result<()> {
    self.child.kill().await?;
    Ok(())
  }
}

/// Find an available port for testing
async fn find_available_port() -> Result<u16> {
  use tokio::net::TcpListener;

  let listener = TcpListener::bind("127.0.0.1:0").await?;
  let port = listener.local_addr()?.port();
  drop(listener); // Release the port
  Ok(port)
}

#[tokio::test]
async fn test_llm_e2e_with_mock_server() -> Result<()> {
  // Initialize logging
  let _ = env_logger::builder()
    .is_test(true)
    .filter_level(log::LevelFilter::Debug)
    .try_init();

  log::info!("=== Starting E2E LLM test ===");

  // 1. Spawn mock LLM server
  log::info!("Step 1: Spawning mock LLM server...");
  let mock_server = MockLlmServer::spawn().await?;
  log::info!("Mock LLM server listening at: {}", mock_server.base_url());

  // 2. Configure Python subprocess to use mock server
  log::info!("Step 2: Configuring Python subprocess...");
  let config = LlmSubprocessConfig::for_testing()
    .with_env("OLLAMA_BASE_URL", format!("{}/v1", mock_server.base_url()));

  // 3. Spawn Python subprocess with AG-UI client (real prod code!)
  log::info!("Step 3: Spawning Python subprocess with AG-UI client...");
  let client = AgUiClient::spawn(config, "ollama", "mock-model").await?;

  log::info!("Python subprocess ready, base URL: {}", client.base_url());

  // 4. Send a test message through the full stack
  log::info!("Step 4: Sending test message...");
  let test_message = "Hello from Rust test!";

  let mut stream = client.chat_stream(test_message, None).await?;

  // 5. Collect streamed response
  log::info!("Step 5: Collecting streamed response...");
  let mut response_text = String::new();
  let mut chunk_count = 0;

  while let Some(result) = stream.next().await {
    match result {
      Ok(chunk) => {
        chunk_count += 1;
        log::debug!("Received chunk {}: {:?}", chunk_count, chunk);
        response_text.push_str(&chunk);
      }
      Err(e) => {
        log::error!("Stream error: {}", e);
        anyhow::bail!("Stream error: {}", e);
      }
    }
  }

  log::info!("Received {} chunks", chunk_count);
  log::info!("Full response: {}", response_text);

  // 6. Verify response
  log::info!("Step 6: Verifying response...");
  assert!(chunk_count > 0, "Should have received at least one chunk");
  assert!(
    response_text.contains("Echo:"),
    "Response should contain 'Echo:'. Got: {}",
    response_text
  );
  assert!(
    response_text.contains(test_message),
    "Response should echo our message. Got: {}",
    response_text
  );

  // 7. Cleanup
  log::info!("Step 7: Cleanup...");
  client.shutdown().await?;
  mock_server.shutdown().await?;

  log::info!("=== E2E LLM test PASSED ===");
  Ok(())
}

#[tokio::test]
async fn test_llm_e2e_multiple_messages() -> Result<()> {
  // Initialize logging
  let _ = env_logger::builder()
    .is_test(true)
    .filter_level(log::LevelFilter::Debug)
    .try_init();

  log::info!("=== Starting multi-message E2E test ===");

  // Spawn mock LLM server
  let mock_server = MockLlmServer::spawn().await?;

  // Spawn Python subprocess
  let config = LlmSubprocessConfig::for_testing()
    .with_env("OLLAMA_BASE_URL", format!("{}/v1", mock_server.base_url()));

  let client = AgUiClient::spawn(config, "ollama", "mock-model").await?;

  // Send multiple messages
  let test_messages = vec![
    "First message",
    "Second message",
    "Third message with special chars: !@#$%",
  ];

  for (i, message) in test_messages.iter().enumerate() {
    log::info!("Sending message {}: {}", i + 1, message);

    let mut stream = client.chat_stream(*message, None).await?;
    let mut response = String::new();

    while let Some(result) = stream.next().await {
      response.push_str(&result?);
    }

    log::info!("Response {}: {}", i + 1, response);

    assert!(
      response.contains("Echo:"),
      "Response {} should contain 'Echo:'",
      i + 1
    );
    assert!(
      response.contains(message),
      "Response {} should echo the message",
      i + 1
    );
  }

  // Cleanup
  client.shutdown().await?;
  mock_server.shutdown().await?;

  log::info!("=== Multi-message E2E test PASSED ===");
  Ok(())
}

#[tokio::test]
async fn test_llm_e2e_error_handling() -> Result<()> {
  // Initialize logging
  let _ = env_logger::builder()
    .is_test(true)
    .filter_level(log::LevelFilter::Debug)
    .try_init();

  log::info!("=== Starting error handling E2E test ===");

  // Spawn mock LLM server
  let mock_server = MockLlmServer::spawn().await?;

  // Spawn Python subprocess
  let config = LlmSubprocessConfig::for_testing()
    .with_env("OLLAMA_BASE_URL", format!("{}/v1", mock_server.base_url()));

  let client = AgUiClient::spawn(config, "ollama", "mock-model").await?;

  // Verify subprocess is running
  assert!(client.is_running().await, "Subprocess should be running");

  // Shutdown mock server while subprocess is still running
  mock_server.shutdown().await?;

  // Wait a moment for the server to fully shut down
  sleep(Duration::from_millis(100)).await;

  // Try to send a message - this should fail gracefully
  log::info!("Attempting to send message with server down...");
  let result = client.chat_stream("test", None).await;

  // The agent spawn might succeed, but streaming should fail
  // or we might get an immediate error - either is acceptable
  match result {
    Ok(mut stream) => {
      // Try to read from the stream
      let first_chunk = stream.next().await;
      match first_chunk {
        Some(Ok(_)) => {
          // Unexpected success - mock server might have buffered
          log::warn!("Unexpectedly got response after server shutdown");
        }
        Some(Err(e)) => {
          log::info!("Got expected error: {}", e);
        }
        None => {
          log::info!("Stream ended without data (acceptable)");
        }
      }
    }
    Err(e) => {
      log::info!("Got expected error on spawn: {}", e);
    }
  }

  // Cleanup
  client.shutdown().await?;

  log::info!("=== Error handling E2E test PASSED ===");
  Ok(())
}
