// Tests for Python LLM Bridge
#[cfg(test)]
mod tests {
  use crate::llm::client::*;
  use crate::llm::{ChatMessage, Provider, TerminalContext};
  use std::path::PathBuf;

  fn skip_if_no_api_key() -> bool {
    std::env::var("ANTHROPIC_API_KEY").is_err()
  }

  #[tokio::test]
  async fn test_bridge_initialization() {
    if skip_if_no_api_key() {
      eprintln!("Skipping test: ANTHROPIC_API_KEY not set");
      return;
    }

    let bridge = LLMClient::new(Provider::Anthropic, None).await;
    assert!(
      bridge.is_ok(),
      "Failed to create bridge: {:?}",
      bridge.err()
    );

    let bridge = bridge.unwrap();
    assert_eq!(bridge.provider(), Provider::Anthropic);
    assert_eq!(bridge.model(), "claude-sonnet-4-5");
  }

  #[tokio::test]
  async fn test_bridge_with_custom_model() {
    if skip_if_no_api_key() {
      eprintln!("Skipping test: ANTHROPIC_API_KEY not set");
      return;
    }

    let bridge = LLMClient::new(
      Provider::Anthropic,
      Some("claude-3-haiku-20240307".to_string()),
    )
    .await;

    assert!(bridge.is_ok());
    let bridge = bridge.unwrap();
    assert_eq!(bridge.model(), "claude-3-haiku-20240307");
  }

  #[tokio::test]
  async fn test_set_cwd() {
    if skip_if_no_api_key() {
      eprintln!("Skipping test: ANTHROPIC_API_KEY not set");
      return;
    }

    let bridge = LLMClient::new(Provider::Anthropic, None).await.unwrap();

    let result = bridge.set_cwd(PathBuf::from("/tmp"));
    assert!(result.is_ok());
  }

  #[tokio::test]
  async fn test_update_scrollback() {
    if skip_if_no_api_key() {
      eprintln!("Skipping test: ANTHROPIC_API_KEY not set");
      return;
    }

    let bridge = LLMClient::new(Provider::Anthropic, None).await.unwrap();

    let lines = vec![
      "line 1".to_string(),
      "line 2".to_string(),
      "line 3".to_string(),
    ];
    let result = bridge.update_scrollback(lines);
    assert!(result.is_ok());
  }

  #[tokio::test]
  async fn test_take_suggested_commands_empty() {
    if skip_if_no_api_key() {
      eprintln!("Skipping test: ANTHROPIC_API_KEY not set");
      return;
    }

    let bridge = LLMClient::new(Provider::Anthropic, None).await.unwrap();

    let commands = bridge.take_suggested_commands();
    assert!(commands.is_ok());
    assert_eq!(commands.unwrap().len(), 0);
  }

  #[tokio::test]
  async fn test_send_message() {
    if skip_if_no_api_key() {
      eprintln!("Skipping test: ANTHROPIC_API_KEY not set");
      return;
    }

    let bridge = LLMClient::new(Provider::Anthropic, None).await.unwrap();

    let context = TerminalContext::new(
      vec!["$ ls".to_string()],
      PathBuf::from("/tmp"),
      Some(0),
    );

    let history: Vec<ChatMessage> = vec![];

    let result = bridge
      .send_message("Reply with just 'OK'", &context, &history)
      .await;

    assert!(result.is_ok(), "Failed to send message: {:?}", result.err());
    let response = result.unwrap();
    // Should get actual LLM response
    assert!(!response.is_empty(), "Response should not be empty");
    println!("LLM Response: {}", response);
  }

  #[tokio::test]
  async fn test_send_message_stream() {
    use futures::StreamExt;

    if skip_if_no_api_key() {
      eprintln!("Skipping test: ANTHROPIC_API_KEY not set");
      return;
    }

    let bridge = LLMClient::new(Provider::Anthropic, None).await.unwrap();

    let context = TerminalContext::new(
      vec!["$ pwd".to_string(), "/tmp".to_string()],
      PathBuf::from("/tmp"),
      Some(0),
    );

    let history: Vec<ChatMessage> = vec![];

    let result = bridge
      .send_message_stream("Reply with just 'STREAMING OK'", &context, &history)
      .await;

    assert!(
      result.is_ok(),
      "Failed to create stream: {:?}",
      result.err()
    );
    let mut stream = result.unwrap();

    // Collect all chunks
    let mut chunks = Vec::new();
    while let Some(chunk_result) = stream.next().await {
      assert!(
        chunk_result.is_ok(),
        "Stream error: {:?}",
        chunk_result.err()
      );
      chunks.push(chunk_result.unwrap());
    }

    // Verify we got chunks
    assert!(!chunks.is_empty(), "Should receive at least one chunk");

    let full_response = chunks.join("");
    println!(
      "Streamed response ({} chunks): {}",
      chunks.len(),
      full_response
    );
    assert!(
      !full_response.is_empty(),
      "Full response should not be empty"
    );
  }
}
