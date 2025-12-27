// Test fixtures for AG-UI subprocess integration tests
//
// Provides utilities for spawning and managing Python subprocess during tests,
// with stderr capture for debugging.

use anyhow::{Context, Result};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::time::{Duration, timeout};

/// Test fixture for managing Python subprocess during tests
///
/// Automatically spawns the subprocess on creation and kills it on drop.
/// Captures stderr output for debugging test failures.
pub struct SubprocessFixture {
  child: Option<Child>,
  port: u16,
  secret: String,
  stderr_lines: Arc<Mutex<Vec<String>>>,
  _stderr_task: tokio::task::JoinHandle<()>,
}

impl SubprocessFixture {
  /// Spawn a new Python subprocess for testing
  ///
  /// This will:
  /// - Find Python/uv in the environment
  /// - Spawn the subprocess with a test secret
  /// - Wait for port discovery
  /// - Start capturing stderr output
  ///
  /// # Panics
  /// Panics if the subprocess fails to start or doesn't provide a port within 10 seconds
  pub async fn new() -> Self {
    Self::new_with_timeout(Duration::from_secs(10)).await
  }

  /// Spawn subprocess with custom timeout
  pub async fn new_with_timeout(startup_timeout: Duration) -> Self {
    // Generate test secret
    let secret = uuid::Uuid::new_v4().to_string();

    // Build command - try uv first, fall back to python
    let python_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .unwrap()
      .join("python");

    let mut cmd = Command::new("uv");
    cmd
      .arg("run")
      .arg("python")
      .arg("-m")
      .arg("terminai_agent")
      .arg("--secret")
      .arg(&secret)
      .current_dir(&python_dir)
      .env("PYTHONUNBUFFERED", "1")
      .stdout(std::process::Stdio::piped())
      .stderr(std::process::Stdio::piped())
      .kill_on_drop(true);

    let mut child = cmd
      .spawn()
      .context("Failed to spawn Python subprocess. Is uv installed?")
      .unwrap();

    // Read stdout for port
    let stdout = child.stdout.take().expect("Failed to get stdout");
    let mut stdout_reader = BufReader::new(stdout).lines();

    let port = timeout(startup_timeout, async {
      while let Ok(Some(line)) = stdout_reader.next_line().await {
        eprintln!("[Test Subprocess] stdout: {}", line);
        if let Some(port_str) = line.strip_prefix("AG_UI_PORT=") {
          return port_str
            .parse::<u16>()
            .context("Failed to parse port")
            .unwrap();
        }
      }
      panic!("Subprocess terminated without providing port");
    })
    .await
    .expect("Timeout waiting for subprocess to start");

    // Capture stderr in background
    let stderr = child.stderr.take().expect("Failed to get stderr");
    let stderr_lines = Arc::new(Mutex::new(Vec::new()));
    let stderr_lines_clone = stderr_lines.clone();

    let stderr_task = tokio::spawn(async move {
      let mut reader = BufReader::new(stderr).lines();
      while let Ok(Some(line)) = reader.next_line().await {
        eprintln!("[Test Subprocess] stderr: {}", line);
        stderr_lines_clone.lock().unwrap().push(line);
      }
    });

    // Give the server a moment to fully start
    tokio::time::sleep(Duration::from_millis(100)).await;

    Self {
      child: Some(child),
      port,
      secret,
      stderr_lines,
      _stderr_task: stderr_task,
    }
  }

  /// Get the port the subprocess is listening on
  pub fn port(&self) -> u16 {
    self.port
  }

  /// Get the shared secret for authentication
  pub fn secret(&self) -> &str {
    &self.secret
  }

  /// Get the base URL for HTTP requests
  pub fn base_url(&self) -> String {
    format!("http://127.0.0.1:{}", self.port)
  }

  /// Get all stderr lines captured so far
  pub fn stderr_lines(&self) -> Vec<String> {
    self.stderr_lines.lock().unwrap().clone()
  }

  /// Print all captured stderr (useful for debugging test failures)
  pub fn print_stderr(&self) {
    let lines = self.stderr_lines();
    if !lines.is_empty() {
      eprintln!("\n=== Python Subprocess Stderr ===");
      for line in lines {
        eprintln!("{}", line);
      }
      eprintln!("================================\n");
    }
  }

  /// Check if subprocess is still running
  pub fn is_running(&mut self) -> bool {
    if let Some(ref mut child) = self.child {
      child.try_wait().ok().flatten().is_none()
    } else {
      false
    }
  }

  /// Shutdown the subprocess gracefully
  pub async fn shutdown(mut self) -> Result<()> {
    if let Some(mut child) = self.child.take() {
      // Try graceful shutdown first
      #[cfg(unix)]
      {
        use nix::sys::signal::{Signal, kill};
        use nix::unistd::Pid;

        if let Some(pid) = child.id() {
          let _ = kill(Pid::from_raw(pid as i32), Signal::SIGTERM);
        }
      }

      #[cfg(windows)]
      {
        let _ = child.kill().await;
      }

      // Wait for exit with timeout
      match timeout(Duration::from_secs(5), child.wait()).await {
        Ok(Ok(status)) => {
          if !status.success() {
            eprintln!("Subprocess exited with status: {}", status);
            self.print_stderr();
          }
          Ok(())
        }
        Ok(Err(e)) => Err(e.into()),
        Err(_) => {
          eprintln!("Timeout waiting for subprocess to exit, killing it");
          let _ = child.kill().await;
          Ok(())
        }
      }
    } else {
      Ok(())
    }
  }
}

impl Drop for SubprocessFixture {
  fn drop(&mut self) {
    if let Some(mut child) = self.child.take() {
      // Kill on drop if not already cleaned up
      let _ = child.start_kill();
    }
  }
}

/// Create a test HTTP client with authentication headers
pub fn create_test_client(fixture: &SubprocessFixture) -> reqwest::Client {
  reqwest::Client::new()
}

/// Create authenticated headers for test requests
pub fn create_auth_headers(
  fixture: &SubprocessFixture,
) -> reqwest::header::HeaderMap {
  let mut headers = reqwest::header::HeaderMap::new();
  headers.insert("x-ag-ui-secret", fixture.secret().parse().unwrap());
  headers.insert("content-type", "application/json".parse().unwrap());
  headers
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn test_fixture_lifecycle() {
    let fixture = SubprocessFixture::new().await;

    assert!(fixture.port() > 0);
    assert!(!fixture.secret().is_empty());
    assert!(fixture.base_url().starts_with("http://"));

    // Verify we can make a request
    let client = create_test_client(&fixture);
    let headers = create_auth_headers(&fixture);

    let response = client
      .get(format!("{}/health", fixture.base_url()))
      .headers(headers)
      .send()
      .await
      .expect("Failed to call health endpoint");

    assert!(response.status().is_success());

    fixture.shutdown().await.expect("Failed to shutdown");
  }

  #[tokio::test]
  async fn test_stderr_capture() {
    let fixture = SubprocessFixture::new().await;

    // Trigger some stderr output by making a request
    let client = create_test_client(&fixture);
    let headers = create_auth_headers(&fixture);

    let _ = client
      .get(format!("{}/", fixture.base_url()))
      .headers(headers)
      .send()
      .await;

    // Give stderr capture a moment
    tokio::time::sleep(Duration::from_millis(100)).await;

    let stderr = fixture.stderr_lines();
    // Should have at least some logging output
    assert!(
      !stderr.is_empty(),
      "Expected some stderr output from Python"
    );

    fixture.shutdown().await.expect("Failed to shutdown");
  }
}
