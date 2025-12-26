/// Quick E2E test for LLM integration with Ollama
use std::path::PathBuf;

#[tokio::main]
async fn main() {
  use futures::StreamExt;
  use termin::llm::{ChatMessage, LLMClient, Provider, TerminalContext};

  println!("=== LLM E2E Integration Test ===\n");

  // Initialize Python (required for PyO3)
  pyo3::prepare_freethreaded_python();

  // Initialize pyo3-async-runtimes with a dedicated runtime
  let pyo3_rt = tokio::runtime::Runtime::new().unwrap();
  let pyo3_rt: &'static tokio::runtime::Runtime = Box::leak(Box::new(pyo3_rt));
  pyo3_async_runtimes::tokio::init_with_runtime(pyo3_rt)
    .expect("Failed to initialize pyo3-async-runtimes");

  println!("✓ Python and async runtimes initialized\n");

  // Create LLM client with Ollama provider (using functiongemma which supports tools)
  println!("Creating LLM client with Ollama (functiongemma model)...");
  let client =
    match LLMClient::new(Provider::Ollama, Some("functiongemma".to_string()))
      .await
    {
      Ok(c) => {
        println!("✓ LLM client created successfully\n");
        c
      }
      Err(e) => {
        eprintln!("✗ Failed to create LLM client: {:#}", e);
        std::process::exit(1);
      }
    };

  // Create test context
  let context = TerminalContext {
    cwd: PathBuf::from("/tmp"),
    history_lines: vec!["echo hello".to_string(), "hello".to_string()],
    last_exit_code: Some(0),
  };

  // Send a simple message
  println!(
    "Sending message to Ollama: 'Say \"test successful\" and nothing else.'\n"
  );
  let mut stream = match client
    .send_message_stream(
      "Say 'test successful' and nothing else.",
      &context,
      &[],
    )
    .await
  {
    Ok(s) => s,
    Err(e) => {
      eprintln!("✗ Failed to start streaming: {:#}", e);
      std::process::exit(1);
    }
  };

  println!("Streaming response:");
  println!("---");

  // Collect the response
  let mut response = String::new();
  let mut chunk_count = 0;

  while let Some(result) = stream.next().await {
    match result {
      Ok(chunk) => {
        chunk_count += 1;
        print!("{}", chunk);
        std::io::Write::flush(&mut std::io::stdout()).ok();
        response.push_str(&chunk);
      }
      Err(e) => {
        eprintln!("\n\n✗ Error during streaming: {:#}", e);
        std::process::exit(1);
      }
    }
  }

  println!("\n---\n");
  println!("✓ Received {} chunks", chunk_count);
  println!("✓ Total response length: {} characters", response.len());

  // Verify we got a response
  if chunk_count == 0 {
    eprintln!("✗ No chunks received!");
    std::process::exit(1);
  }

  if response.is_empty() {
    eprintln!("✗ Response is empty!");
    std::process::exit(1);
  }

  println!("\n=== ALL TESTS PASSED ===");
}
