// TERMIN.AI: Transport layer for AG-UI protocol
// Handles HTTP communication, authentication, and AG-UI mechanics
//
// NOTE: This is a simplified version due to limited exports in published ag-ui-client 0.1.0
// Full AG-UI integration will be added when ag-ui-client 0.2+ is published

use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::str::FromStr;
use tokio::sync::Mutex;

use crate::llm_subprocess::{LlmSubprocess, LlmSubprocessConfig};

/// Transport layer for AG-UI communication
///
/// This handles the low-level mechanics of connecting to the Python subprocess
/// via HTTP + AG-UI protocol. It abstracts away authentication, headers, SSE,
/// and the subprocess lifecycle.
pub struct AgUiTransport {
  /// Base URL for the agent
  base_url: String,
  /// Headers including shared secret
  headers: HeaderMap,
  /// Python subprocess handle
  subprocess: Mutex<Option<LlmSubprocess>>,
}

impl AgUiTransport {
  /// Create a new AG-UI transport by spawning the Python subprocess
  ///
  /// This will:
  /// 1. Spawn the Python subprocess with a shared secret
  /// 2. Wait for port discovery via stdout
  /// 3. Store connection details for HTTP requests
  ///
  /// # Arguments
  /// * `config` - Subprocess configuration (port range, host, etc.)
  ///
  /// # Returns
  /// Configured transport ready for use
  pub async fn spawn(config: LlmSubprocessConfig) -> Result<Self> {
    log::info!("Spawning LLM subprocess and creating AG-UI transport");

    // Spawn the subprocess
    let subprocess = LlmSubprocess::spawn(config).await?;

    // Build headers with shared secret
    let mut headers = HeaderMap::new();
    headers.insert(
      HeaderName::from_str("x-ag-ui-secret")?,
      HeaderValue::from_str(subprocess.secret())?,
    );
    headers.insert(
      HeaderName::from_str("content-type")?,
      HeaderValue::from_static("application/json"),
    );

    let base_url = subprocess.base_url().to_string();

    log::info!("AG-UI transport created, connected to {}", base_url);

    Ok(Self {
      base_url,
      headers,
      subprocess: Mutex::new(Some(subprocess)),
    })
  }

  /// Get the base URL for the agent
  pub fn base_url(&self) -> &str {
    &self.base_url
  }

  /// Get the headers (including auth) for requests
  pub fn headers(&self) -> &HeaderMap {
    &self.headers
  }

  /// Check if the subprocess is still running
  pub async fn is_subprocess_running(&self) -> bool {
    if let Some(subprocess) = self.subprocess.lock().await.as_ref() {
      subprocess.is_running().await
    } else {
      false
    }
  }

  /// Shutdown the transport and Python subprocess gracefully
  pub async fn shutdown(self) -> Result<()> {
    log::info!("Shutting down AG-UI transport");

    if let Some(subprocess) = self.subprocess.lock().await.take() {
      subprocess
        .shutdown()
        .await
        .context("Failed to shutdown subprocess")?;
    }

    Ok(())
  }
}

impl Drop for AgUiTransport {
  fn drop(&mut self) {
    log::debug!("AgUiTransport dropped");
    // Subprocess will be killed via kill_on_drop(true)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn test_transport_lifecycle() {
    let config = LlmSubprocessConfig::for_testing();
    let transport = AgUiTransport::spawn(config)
      .await
      .expect("Failed to spawn transport");

    assert!(transport.is_subprocess_running().await);
    assert!(!transport.base_url().is_empty());
    assert!(transport.headers().contains_key("x-ag-ui-secret"));

    transport
      .shutdown()
      .await
      .expect("Failed to shutdown transport");
  }
}
