// End-to-end test for AG-UI tool call integration with real LLM
//
// This test exercises the full flow:
// 1. Rust spawns Python subprocess (real prod code)
// 2. Python subprocess uses real Anthropic Claude Haiku 4.5 API
// 3. Rust sends messages via AG-UI client (real prod code)
// 4. LLM calls tools (read_scrollback, suggest_command)
// 5. Tool results are returned to LLM
// 6. LLM suggests a command based on terminal context
// 7. We verify the command suggestion is correct
//
// This test requires ANTHROPIC_API_KEY to be set in the environment.

use anyhow::Result;
use futures::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::llm::{
  AgUiClient, CommandSuggestion, Message, TerminalContext, ToolCoordinator,
  ToolExecutionContext, ToolExecutionEvent, ToolExecutor,
  run_tool_execution_loop,
};
use crate::llm_subprocess::LlmSubprocessConfig;

/// Test the full E2E flow using real Anthropic API
///
/// This test:
/// 1. Sets up terminal context with scrollback suggesting "Hello, World" goal
/// 2. Sends a user message asking to suggest a command
/// 3. Expects LLM to call read_scrollback and then suggest_command
/// 4. Verifies the suggested command contains "Hello, World"
#[tokio::test]
#[ignore] // Run with: cargo test test_agui_tool_e2e_real_anthropic -- --ignored
async fn test_agui_tool_e2e_real_anthropic() -> Result<()> {
  // Initialize logging
  let _ = env_logger::builder()
    .is_test(true)
    .filter_level(log::LevelFilter::Debug)
    .try_init();

  log::info!("=== Starting AG-UI Tool E2E test with real Anthropic API ===");

  // Check for API key
  if std::env::var("ANTHROPIC_API_KEY").is_err() {
    log::warn!("ANTHROPIC_API_KEY not set, skipping test");
    return Ok(());
  }

  // 1. Spawn Python subprocess with Anthropic provider
  log::info!("Step 1: Spawning Python subprocess with Anthropic provider...");
  let config = LlmSubprocessConfig::for_testing();
  let client = Arc::new(
    AgUiClient::spawn(config, "anthropic", "claude-3-5-haiku-latest").await?,
  );

  log::info!("Python subprocess ready, base URL: {}", client.base_url());

  // 2. Set up terminal context with scrollback suggesting the goal
  log::info!("Step 2: Setting up terminal context with Hello, World goal...");
  let terminal_context = TerminalContext {
    history_lines: vec![
      "$ pwd".to_string(),
      "/home/user/projects".to_string(),
      "$ # My goal is to print 'Hello, World!' to the terminal".to_string(),
      "$ # I need help with the command to do this".to_string(),
    ],
    cwd: "/home/user/projects".to_string(),
    last_exit_code: Some(0),
    os_info: Some("Linux".to_string()),
    shell: Some("bash".to_string()),
  };

  // 3. Set up tool execution infrastructure
  log::info!("Step 3: Setting up tool execution infrastructure...");
  let command_suggestions =
    Arc::new(Mutex::new(Vec::<CommandSuggestion>::new()));
  let message_history = Arc::new(Mutex::new(Vec::<Message>::new()));

  // Create tool executor with fallback scrollback (no real VT parser in test)
  let tool_context = ToolExecutionContext {
    vt_parser: None,
    fallback_scrollback: Some(terminal_context.history_lines.clone()),
    command_suggestions: Arc::clone(&command_suggestions),
    command_executor: crate::command::CommandExecutor::new(),
    safety_validator: crate::command::SafetyValidator::new(),
  };
  let tool_executor = ToolExecutor::new(tool_context);

  // Create tool coordinator
  let coordinator = Arc::new(ToolCoordinator::new(
    Arc::clone(&client),
    tool_executor,
    Arc::clone(&message_history),
    Arc::clone(&command_suggestions),
  ));

  // 4. Send message to LLM
  log::info!("Step 4: Sending message to LLM...");
  let user_message = "Read my recent terminal history and suggest what command I should call to achieve my goal. Use the suggest_command tool.";

  // Add user message to history
  {
    let mut history = message_history.lock().await;
    history.push(Message::User {
      id: ag_ui_core::types::ids::MessageId::random(),
      content: user_message.to_string(),
      name: None,
    });
  }

  // Start streaming response
  let response = client
    .chat_stream(
      user_message,
      Some(Arc::clone(&message_history)),
      Some(&terminal_context),
    )
    .await?;

  let mut text_stream = response.text_stream;
  let tool_rx = response.tool_rx;

  // 5. Spawn tool execution loop in background
  log::info!("Step 5: Starting tool execution loop...");
  let (event_tx, mut event_rx) =
    tokio::sync::mpsc::unbounded_channel::<ToolExecutionEvent>();
  let coordinator_clone = Arc::clone(&coordinator);

  let tool_task = tokio::spawn(async move {
    run_tool_execution_loop(coordinator_clone, tool_rx, event_tx).await;
  });

  // 6. Collect streamed response with timeout
  log::info!("Step 6: Collecting streamed response...");
  let mut response_text = String::new();
  let timeout = Duration::from_secs(60); // Generous timeout for LLM response
  let timeout_instant = tokio::time::Instant::now() + timeout;

  loop {
    tokio::select! {
      result = text_stream.next() => {
        match result {
          Some(Ok(chunk)) => {
            log::debug!("Received text chunk: {:?}", chunk);
            response_text.push_str(&chunk);
          }
          Some(Err(e)) => {
            log::error!("Stream error: {}", e);
            break;
          }
          None => {
            log::info!("Text stream ended");
            break;
          }
        }
      }
      tool_event = event_rx.recv() => {
        match tool_event {
          Some(ToolExecutionEvent::ToolExecuted { tool_name, .. }) => {
            log::info!("Tool executed: {}", tool_name);
          }
          Some(ToolExecutionEvent::ContinuedTextChunk { chunk }) => {
            log::debug!("Continued text chunk: {:?}", chunk);
            response_text.push_str(&chunk);
          }
          Some(ToolExecutionEvent::ContinuedStreamComplete { full_response }) => {
            log::info!("Continued stream complete: {} chars", full_response.len());
          }
          Some(ToolExecutionEvent::Error { message }) => {
            log::error!("Tool execution error: {}", message);
            anyhow::bail!("Tool execution error: {}", message);
          }
          Some(ToolExecutionEvent::ToolCallStarted { tool_name, .. }) => {
            log::info!("Tool call started: {}", tool_name);
          }
          Some(ToolExecutionEvent::ToolFailed { tool_name, error_message, .. }) => {
            log::error!("Tool failed: {} - {}", tool_name, error_message);
          }
          None => {
            log::info!("Event channel closed");
          }
        }
      }
      _ = tokio::time::sleep_until(timeout_instant) => {
        log::warn!("Timeout waiting for response");
        break;
      }
    }
  }

  // Allow tool task to complete (5s for continued stream with nested tool calls)
  tokio::time::sleep(Duration::from_secs(5)).await;
  tool_task.abort();

  // 7. Verify command suggestion
  log::info!("Step 7: Verifying command suggestion...");
  let suggestions = command_suggestions.lock().await;

  log::info!("Full response: {}", response_text);
  log::info!("Command suggestions: {:?}", *suggestions);

  // Assert that we got a suggestion containing "Hello, World"
  assert!(
    !suggestions.is_empty(),
    "Expected at least one command suggestion, got none. Response: {}",
    response_text
  );

  let has_hello_world = suggestions
    .iter()
    .any(|s| s.command.contains("Hello") || s.command.contains("hello"));

  assert!(
    has_hello_world,
    "Expected suggestion to contain 'Hello, World'. Got suggestions: {:?}",
    *suggestions
  );

  // 8. Cleanup
  log::info!("Step 8: Cleanup...");
  drop(suggestions);
  // Client shutdown is handled by Arc drop

  log::info!("=== AG-UI Tool E2E test PASSED ===");
  Ok(())
}

/// Test tool call flow with mock server to isolate tool execution issues
#[tokio::test]
async fn test_agui_tool_call_flow_with_mock() -> Result<()> {
  // Initialize logging
  let _ = env_logger::builder()
    .is_test(true)
    .filter_level(log::LevelFilter::Debug)
    .try_init();

  log::info!("=== Starting AG-UI Tool Call Flow test with mock server ===");

  // Spawn mock LLM server
  let mock_server = MockLlmServer::spawn().await?;
  log::info!("Mock LLM server listening at: {}", mock_server.base_url());

  // Spawn Python subprocess
  let config = LlmSubprocessConfig::for_testing()
    .with_env("OLLAMA_BASE_URL", format!("{}/v1", mock_server.base_url()));

  let client =
    Arc::new(AgUiClient::spawn(config, "ollama", "mock-model").await?);

  // Set up tool execution infrastructure
  let command_suggestions =
    Arc::new(Mutex::new(Vec::<CommandSuggestion>::new()));
  let message_history = Arc::new(Mutex::new(Vec::<Message>::new()));

  // Set up terminal context
  let terminal_context = TerminalContext {
    history_lines: vec!["$ # Goal: print Hello, World!".to_string()],
    cwd: "/home/test".to_string(),
    last_exit_code: Some(0),
    os_info: Some("Linux".to_string()),
    shell: Some("bash".to_string()),
  };

  // Create tool executor with fallback scrollback
  let tool_context = ToolExecutionContext {
    vt_parser: None,
    fallback_scrollback: Some(terminal_context.history_lines.clone()),
    command_suggestions: Arc::clone(&command_suggestions),
    command_executor: crate::command::CommandExecutor::new(),
    safety_validator: crate::command::SafetyValidator::new(),
  };
  let tool_executor = ToolExecutor::new(tool_context);

  // Create tool coordinator
  let coordinator = Arc::new(ToolCoordinator::new(
    Arc::clone(&client),
    tool_executor,
    Arc::clone(&message_history),
    Arc::clone(&command_suggestions),
  ));

  // Add user message to history
  let user_message =
    "[TOOL:suggest_command|echo 'Hello, World!'|Print greeting]";
  {
    let mut history = message_history.lock().await;
    history.push(Message::User {
      id: ag_ui_core::types::ids::MessageId::random(),
      content: user_message.to_string(),
      name: None,
    });
  }

  // Start streaming response
  let response = client
    .chat_stream(
      user_message,
      Some(Arc::clone(&message_history)),
      Some(&terminal_context),
    )
    .await?;

  let mut text_stream = response.text_stream;
  let tool_rx = response.tool_rx;

  // Spawn tool execution loop
  let (event_tx, mut event_rx) =
    tokio::sync::mpsc::unbounded_channel::<ToolExecutionEvent>();
  let coordinator_clone = Arc::clone(&coordinator);

  let tool_task = tokio::spawn(async move {
    run_tool_execution_loop(coordinator_clone, tool_rx, event_tx).await;
  });

  // Collect response
  let mut response_text = String::new();
  let mut tool_executed = false;
  let timeout = Duration::from_secs(30);
  let timeout_instant = tokio::time::Instant::now() + timeout;

  loop {
    tokio::select! {
      result = text_stream.next() => {
        match result {
          Some(Ok(chunk)) => {
            response_text.push_str(&chunk);
          }
          Some(Err(e)) => {
            log::error!("Stream error: {}", e);
            break;
          }
          None => {
            log::info!("Text stream ended");
            break;
          }
        }
      }
      tool_event = event_rx.recv() => {
        match tool_event {
          Some(ToolExecutionEvent::ToolExecuted { tool_name, .. }) => {
            log::info!("✅ Tool executed: {}", tool_name);
            tool_executed = true;
            if tool_name == "suggest_command" {
              // Tool executed successfully
            }
          }
          Some(ToolExecutionEvent::ContinuedTextChunk { chunk }) => {
            response_text.push_str(&chunk);
          }
          Some(ToolExecutionEvent::ContinuedStreamComplete { .. }) => {
            log::info!("Continued stream complete");
          }
          Some(ToolExecutionEvent::Error { message }) => {
            log::error!("❌ Tool execution error: {}", message);
            anyhow::bail!("Tool execution error: {}", message);
          }
          Some(ToolExecutionEvent::ToolCallStarted { tool_name, .. }) => {
            log::info!("Tool call started: {}", tool_name);
          }
          Some(ToolExecutionEvent::ToolFailed { tool_name, error_message, .. }) => {
            log::error!("Tool failed: {} - {}", tool_name, error_message);
          }
          None => {
            log::info!("Event channel closed");
            break;
          }
        }
      }
      _ = tokio::time::sleep_until(timeout_instant) => {
        log::warn!("Timeout waiting for response");
        break;
      }
    }
  }

  // Wait briefly for tool execution to complete
  tokio::time::sleep(Duration::from_millis(500)).await;
  tool_task.abort();

  // Verify results
  log::info!("Response: {}", response_text);
  let suggestions = command_suggestions.lock().await;
  log::info!("Suggestions: {:?}", *suggestions);

  // Assert we got a suggestion containing "Hello, World"
  assert!(
    !suggestions.is_empty(),
    "Expected at least one command suggestion"
  );

  let has_hello_world = suggestions
    .iter()
    .any(|s| s.command.contains("Hello") && s.command.contains("World"));

  assert!(
    has_hello_world,
    "Expected suggestion to contain 'Hello, World'. Got: {:?}",
    suggestions.iter().map(|s| &s.command).collect::<Vec<_>>()
  );

  log::info!("✅ Test PASSED: Suggestion contains 'Hello, World'");

  // Cleanup
  drop(suggestions);
  mock_server.shutdown().await?;

  log::info!("=== AG-UI Tool Call Flow test completed ===");
  Ok(())
}

/// Helper to spawn the mock LLM server (copied from test_llm_e2e.rs)
struct MockLlmServer {
  child: tokio::process::Child,
  port: u16,
}

impl MockLlmServer {
  async fn spawn() -> Result<Self> {
    use std::process::Stdio;
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::process::Command;
    use tokio::time::sleep;

    // Find an available port
    let port = {
      let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
      listener.local_addr()?.port()
    };

    // Get Python directory
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
      .spawn()?;

    // Monitor stderr for startup
    let stderr = child.stderr.take().expect("Failed to capture stderr");
    let mut stderr_reader = BufReader::new(stderr).lines();

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

  fn base_url(&self) -> String {
    format!("http://127.0.0.1:{}", self.port)
  }

  async fn shutdown(mut self) -> Result<()> {
    self.child.kill().await?;
    Ok(())
  }
}
