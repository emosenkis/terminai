// TERMIN.AI: Python subprocess management for LLM agent

use anyhow::{Context, Result};
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::time::{Duration, timeout};
use uuid::Uuid;

/// Configuration for the LLM agent subprocess
#[derive(Debug, Clone)]
pub struct LlmSubprocessConfig {
  /// Python interpreter command (usually "python" or "python3")
  pub python_command: String,
  /// Port range to try for the server
  pub port_range: (u16, u16),
  /// Host to bind to
  pub host: String,
  /// Optional explicit Python project directory (for tests)
  pub python_dir: Option<std::path::PathBuf>,
}

impl Default for LlmSubprocessConfig {
  fn default() -> Self {
    Self {
      python_command: "python".to_string(),
      port_range: (18080, 18099),
      host: "127.0.0.1".to_string(),
      python_dir: None,
    }
  }
}

impl LlmSubprocessConfig {
  /// Create a config for testing with explicit Python directory
  #[cfg(test)]
  pub fn for_testing() -> Self {
    Self {
      python_command: "python".to_string(),
      port_range: (18080, 18099),
      host: "127.0.0.1".to_string(),
      python_dir: Some(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
          .parent()
          .unwrap()
          .join("python"),
      ),
    }
  }
}

/// Handle to the running LLM agent subprocess
pub struct LlmSubprocess {
  /// The child process
  child: Arc<Mutex<Child>>,
  /// Shared secret for authentication
  secret: String,
  /// Port the server is listening on
  port: u16,
  /// Base URL for the server
  base_url: String,
}

impl LlmSubprocess {
  /// Spawn the LLM agent subprocess
  pub async fn spawn(config: LlmSubprocessConfig) -> Result<Self> {
    // Generate shared secret
    let secret = Uuid::new_v4().to_string();

    // Find the Python project directory
    let python_dir = if let Some(dir) = config.python_dir {
      // Explicit directory provided (typically for tests)
      dir
    } else {
      // Auto-detect: try relative to executable, then current dir
      std::env::current_exe()?
        .parent()
        .context("Could not determine executable directory")?
        .join("../python")
        .canonicalize()
        .or_else(|_| {
          // Fallback to workspace-relative path for development
          std::env::current_dir()?
            .join("python")
            .canonicalize()
            .context("Could not find Python project directory")
        })?
    };

    log::info!("Python project directory: {}", python_dir.display());

    // Spawn the subprocess
    // NOTE: The subprocess inherits ALL environment variables from the parent process,
    // including those loaded from ~/.config/terminai/terminai.env
    // This allows users to configure API keys and other settings via standard env vars
    log::info!("Spawning LLM agent subprocess");
    let mut child = Command::new("uv")
      .arg("run")
      .arg("python")
      .arg("-m")
      .arg("terminai_agent")
      .arg("--secret")
      .arg(&secret)
      .arg("--port-range-start")
      .arg(config.port_range.0.to_string())
      .arg("--port-range-end")
      .arg(config.port_range.1.to_string())
      .arg("--host")
      .arg(&config.host)
      .current_dir(&python_dir)
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .kill_on_drop(true)
      .spawn()
      .context("Failed to spawn Python subprocess")?;

    // Read stdout and stderr concurrently
    let stdout = child.stdout.take().context("Failed to capture stdout")?;
    let stderr = child.stderr.take().context("Failed to capture stderr")?;

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    log::debug!("Waiting for AG_UI_PORT from subprocess...");

    // Collect stderr lines in case of failure
    let stderr_lines = Arc::new(Mutex::new(Vec::new()));
    let stderr_lines_clone = stderr_lines.clone();

    // Spawn task to capture stderr
    let stderr_task = tokio::spawn(async move {
      while let Ok(Some(line)) = stderr_reader.next_line().await {
        log::info!("[Python stderr] {}", line);
        stderr_lines_clone.lock().await.push(line);
      }
    });

    let port = timeout(Duration::from_secs(10), async {
      while let Some(line) = stdout_reader.next_line().await? {
        log::debug!("Subprocess stdout: {}", line);
        if let Some(port_str) = line.strip_prefix("AG_UI_PORT=") {
          let port: u16 =
            port_str.parse().context("Failed to parse port number")?;
          return Ok::<u16, anyhow::Error>(port);
        }
      }

      // Subprocess terminated - check if it's still running
      let status = child.try_wait()?;
      if let Some(exit_status) = status {
        // Process exited - wait a moment for stderr to be captured
        tokio::time::sleep(Duration::from_millis(100)).await;
        let stderr_output = stderr_lines.lock().await.join("\n");
        if stderr_output.is_empty() {
          anyhow::bail!("Subprocess terminated with status {} without providing port", exit_status);
        } else {
          anyhow::bail!(
            "Subprocess terminated with status {} without providing port.\nStderr:\n{}",
            exit_status,
            stderr_output
          );
        }
      }

      anyhow::bail!("Subprocess terminated without providing port")
    })
    .await
    .context("Timeout waiting for subprocess to start")??;

    log::info!("LLM agent subprocess started on port {}", port);

    // Continue monitoring stderr after successful startup
    tokio::spawn(async move {
      let _ = stderr_task.await;
      log::debug!("Python subprocess stderr monitoring ended");
    });

    let base_url = format!("http://{}:{}", config.host, port);

    Ok(Self {
      child: Arc::new(Mutex::new(child)),
      secret,
      port,
      base_url,
    })
  }

  /// Get the port the server is listening on
  pub fn port(&self) -> u16 {
    self.port
  }

  /// Get the shared secret for authentication
  pub fn secret(&self) -> &str {
    &self.secret
  }

  /// Get the base URL for the server
  pub fn base_url(&self) -> &str {
    &self.base_url
  }

  /// Check if the subprocess is still running
  pub async fn is_running(&self) -> bool {
    let mut child = self.child.lock().await;
    child.try_wait().ok().flatten().is_none()
  }

  /// Wait for the subprocess to exit
  pub async fn wait(&self) -> Result<std::process::ExitStatus> {
    let mut child = self.child.lock().await;
    child.wait().await.context("Failed to wait for subprocess")
  }

  /// Shutdown the subprocess gracefully
  pub async fn shutdown(&self) -> Result<()> {
    log::info!("Shutting down LLM agent subprocess");
    let mut child = self.child.lock().await;

    // Try graceful shutdown via HTTP (future enhancement)
    // For now, just kill the process

    // Send SIGTERM (on Unix) or TerminateProcess (on Windows)
    #[cfg(unix)]
    {
      use nix::sys::signal::{Signal, kill};
      use nix::unistd::Pid;

      if let Some(pid) = child.id() {
        let pid = Pid::from_raw(pid as i32);
        let _ = kill(pid, Signal::SIGTERM);
        log::debug!("Sent SIGTERM to subprocess");
      }
    }

    #[cfg(not(unix))]
    {
      let _ = child.kill().await;
    }

    // Wait up to 5 seconds for graceful shutdown
    match timeout(Duration::from_secs(5), child.wait()).await {
      Ok(Ok(status)) => {
        log::info!("Subprocess exited with status: {}", status);
        Ok(())
      }
      Ok(Err(e)) => {
        log::error!("Error waiting for subprocess: {}", e);
        Err(e.into())
      }
      Err(_) => {
        log::warn!("Subprocess did not exit gracefully, force-killing");
        child.kill().await?;
        Ok(())
      }
    }
  }
}

impl Drop for LlmSubprocess {
  fn drop(&mut self) {
    log::debug!("LlmSubprocess dropped, subprocess will be killed");
    // The process will be killed automatically due to kill_on_drop(true)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  #[ignore] // Requires Python environment
  async fn test_subprocess_lifecycle() {
    let config = LlmSubprocessConfig::default();
    let subprocess = LlmSubprocess::spawn(config)
      .await
      .expect("Failed to spawn subprocess");

    assert!(subprocess.is_running().await);
    assert!(subprocess.port() > 0);
    assert!(!subprocess.secret().is_empty());
    assert!(subprocess.base_url().starts_with("http://"));

    subprocess
      .shutdown()
      .await
      .expect("Failed to shutdown subprocess");
  }
}
