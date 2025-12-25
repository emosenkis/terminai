// Example: Using the LLM Client Adapter
//
// This example demonstrates how to use the LLMClientAdapter which
// can switch between Rig and Python backends based on feature flags.
//
// Usage:
//   # With Rig backend (default)
//   cargo run --example llm_adapter_example
//
//   # With Python backend
//   PYO3_PYTHON=python/.venv/bin/python cargo run --example llm_adapter_example --features python-llm
//
// Requirements:
//   - Set ANTHROPIC_API_KEY environment variable
//   - Python environment set up (for python-llm feature)

use anyhow::Result;
use std::path::PathBuf;
use termin::llm::{ChatMessage, LLMClientAdapter, Provider, TerminalContext};

#[tokio::main]
async fn main() -> Result<()> {
  // Check for API key
  if std::env::var("ANTHROPIC_API_KEY").is_err() {
    eprintln!("Error: ANTHROPIC_API_KEY environment variable not set");
    eprintln!("\nPlease set your Anthropic API key:");
    eprintln!("  export ANTHROPIC_API_KEY=sk-...");
    std::process::exit(1);
  }

  println!("LLM Client Adapter Example");
  println!("==========================\n");

  #[cfg(feature = "python-llm")]
  println!("✓ Using Python backend (via PydanticAI)");

  #[cfg(not(feature = "python-llm"))]
  println!("✓ Using Rig backend");

  // Create the adapter
  println!("\n1. Creating LLM client adapter...");
  let client = LLMClientAdapter::new(Provider::Anthropic, None).await?;
  println!("   Created successfully!");

  // Set up context
  println!("\n2. Setting up terminal context...");
  let context = TerminalContext::new(
    vec![
      "$ pwd".to_string(),
      "/home/user/projects".to_string(),
      "$ ls -la".to_string(),
      "total 42".to_string(),
      "drwxr-xr-x 5 user user 4096 Dec 25 10:30 .".to_string(),
      "drwxr-xr-x 3 user user 4096 Dec 20 09:15 ..".to_string(),
      "-rw-r--r-- 1 user user 1234 Dec 25 10:30 README.md".to_string(),
      "drwxr-xr-x 2 user user 4096 Dec 25 10:25 src".to_string(),
    ],
    PathBuf::from("/home/user/projects"),
    Some(0),
  );
  println!("   Context created with {} history lines", context.history_lines.len());

  // Update client context
  client.set_cwd(context.cwd.clone())?;
  client.update_scrollback(context.history_lines.clone())?;

  // Send a message
  println!("\n3. Sending message to LLM...");
  let user_message = "What files are in my current directory?";
  println!("   User: {}", user_message);

  let history: Vec<ChatMessage> = vec![];

  println!("\n   Waiting for response...");
  let response = client.send_message(user_message, &context, &history).await?;

  println!("\n   Assistant: {}", response);

  // Check for suggested commands
  println!("\n4. Checking for suggested commands...");
  let commands = client.take_suggested_commands()?;

  if commands.is_empty() {
    println!("   No commands suggested");
  } else {
    println!("   Found {} suggested command(s):", commands.len());
    for (i, cmd) in commands.iter().enumerate() {
      println!("\n   Command {}:", i + 1);
      println!("     > {}", cmd.command);
      println!("     Explanation: {}", cmd.explanation);
      println!("     Raw: {}", cmd.raw);
    }
  }

  println!("\n5. Testing streaming (if supported)...");
  let mut full_response = String::new();
  let mut stream = client
    .send_message_stream(
      "How do I create a new directory?",
      &context,
      &[ChatMessage::user(user_message.to_string()), ChatMessage::assistant(response)],
    )
    .await?;

  use futures::StreamExt;
  while let Some(chunk) = stream.next().await {
    match chunk {
      Ok(text) => {
        full_response.push_str(&text);
        // In a real UI, you'd display this incrementally
      }
      Err(e) => {
        eprintln!("   Stream error: {}", e);
        break;
      }
    }
  }

  if full_response.is_empty() {
    println!("   No streaming response (may not be supported by backend)");
  } else {
    println!("   Streaming response received ({} chars)", full_response.len());
  }

  println!("\n✓ Example completed successfully!");
  println!("\nNote: The Python backend currently returns a single-item stream.");
  println!("True streaming support requires pyo3-async-runtimes integration.");

  Ok(())
}
