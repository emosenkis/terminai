// Test configuration for e2e testing
// This module provides mock LLM clients and test configurations

use anyhow::Result;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::llm::{AgUiTerminalContext, Provider};

/// Test configuration for LLM behavior
#[derive(Debug, Clone)]
pub struct TestLLMConfig {
  /// Canned responses to return for specific prompts
  pub responses: Vec<String>,
  /// Current response index
  response_index: Arc<Mutex<usize>>,
  /// Whether to simulate streaming
  pub simulate_streaming: bool,
  /// Simulated delay in milliseconds
  pub delay_ms: u64,
}

impl TestLLMConfig {
  pub fn new(responses: Vec<String>) -> Self {
    Self {
      responses,
      response_index: Arc::new(Mutex::new(0)),
      simulate_streaming: false,
      delay_ms: 0,
    }
  }

  pub fn with_streaming(mut self, enabled: bool) -> Self {
    self.simulate_streaming = enabled;
    self
  }

  pub fn with_delay(mut self, delay_ms: u64) -> Self {
    self.delay_ms = delay_ms;
    self
  }

  /// Get the next canned response
  pub fn next_response(&self) -> Option<String> {
    let mut idx = self.response_index.lock().unwrap();
    if *idx < self.responses.len() {
      let response = self.responses[*idx].clone();
      *idx += 1;
      Some(response)
    } else {
      None
    }
  }

  /// Reset response index
  pub fn reset(&self) {
    let mut idx = self.response_index.lock().unwrap();
    *idx = 0;
  }
}

/// Mock LLM client for testing
pub struct MockLLMClient {
  config: TestLLMConfig,
  provider: Provider,
  model: String,
}

impl MockLLMClient {
  pub fn new(config: TestLLMConfig, provider: Provider) -> Self {
    Self {
      config,
      provider: provider.clone(),
      model: provider.default_model().to_string(),
    }
  }

  /// Simulate sending a message and getting a response
  pub async fn send_message(
    &self,
    _user_message: &str,
    _context: Option<AgUiTerminalContext>,
  ) -> Result<String> {
    // Simulate delay if configured
    if self.config.delay_ms > 0 {
      tokio::time::sleep(tokio::time::Duration::from_millis(
        self.config.delay_ms,
      ))
      .await;
    }

    // Return next canned response
    self
      .config
      .next_response()
      .ok_or_else(|| anyhow::anyhow!("No more canned responses available"))
  }

  pub fn provider(&self) -> Provider {
    self.provider
  }

  pub fn model(&self) -> &str {
    &self.model
  }
}

/// Test application configuration
#[derive(Debug, Clone)]
pub struct TestAppConfig {
  /// Shell command to use (defaults to "sh" or "bash")
  pub shell_cmd: String,
  /// Initial working directory
  pub cwd: PathBuf,
  /// LLM configuration for AI testing
  pub llm_config: Option<TestLLMConfig>,
  /// Terminal size (width, height)
  pub terminal_size: (u16, u16),
}

impl Default for TestAppConfig {
  fn default() -> Self {
    Self {
      shell_cmd: std::env::var("SHELL")
        .unwrap_or_else(|_| "/bin/sh".to_string()),
      cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
      llm_config: None,
      terminal_size: (80, 24),
    }
  }
}

impl TestAppConfig {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn with_shell(mut self, shell_cmd: impl Into<String>) -> Self {
    self.shell_cmd = shell_cmd.into();
    self
  }

  pub fn with_cwd(mut self, cwd: PathBuf) -> Self {
    self.cwd = cwd;
    self
  }

  pub fn with_llm_config(mut self, llm_config: TestLLMConfig) -> Self {
    self.llm_config = Some(llm_config);
    self
  }

  pub fn with_terminal_size(mut self, width: u16, height: u16) -> Self {
    self.terminal_size = (width, height);
    self
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_llm_config_responses() {
    let config = TestLLMConfig::new(vec![
      "Response 1".to_string(),
      "Response 2".to_string(),
    ]);

    assert_eq!(config.next_response(), Some("Response 1".to_string()));
    assert_eq!(config.next_response(), Some("Response 2".to_string()));
    assert_eq!(config.next_response(), None);

    config.reset();
    assert_eq!(config.next_response(), Some("Response 1".to_string()));
  }

  #[tokio::test]
  async fn test_mock_llm_client() {
    let config = TestLLMConfig::new(vec!["Hello from AI".to_string()]);
    let client = MockLLMClient::new(config, Provider::Anthropic);

    let response = client.send_message("Test", None).await.unwrap();
    assert_eq!(response, "Hello from AI");
  }

  #[test]
  fn test_app_config_builder() {
    let config = TestAppConfig::new()
      .with_shell("bash")
      .with_terminal_size(120, 40);

    assert_eq!(config.shell_cmd, "bash");
    assert_eq!(config.terminal_size, (120, 40));
  }
}
