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
  /// Optional environment variables to set for the subprocess (for tests)
  pub env_vars: Vec<(String, String)>,
}

impl Default for LlmSubprocessConfig {
  fn default() -> Self {
    Self {
      python_command: "python".to_string(),
      port_range: (18080, 18099),
      host: "127.0.0.1".to_string(),
      python_dir: None,
      env_vars: Vec::new(),
    }
  }
}

impl LlmSubprocessConfig {
  /// Create a config for testing with explicit Python directory
  pub fn for_testing() -> Self {
    Self {
      python_command: "python".to_string(),
      port_range: (18080, 18099),
      host: "127.0.0.1".to_string(),
      python_dir: Some(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
          .parent()
          .expect("Failed to get parent directory")
          .join("python"),
      ),
      env_vars: Vec::new(),
    }
  }

  /// Add an environment variable to the subprocess
  pub fn with_env(
    mut self,
    key: impl Into<String>,
    value: impl Into<String>,
  ) -> Self {
    self.env_vars.push((key.into(), value.into()));
    self
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
      log::info!(
        "Using explicit Python directory from config: {}",
        dir.display()
      );
      dir
    } else if let Ok(env_dir) = std::env::var("TERMINAI_PYTHON_DIR") {
      // Environment variable takes precedence (set by package manager wrappers)
      let path = std::path::PathBuf::from(&env_dir);
      log::info!(
        "Using Python directory from TERMINAI_PYTHON_DIR: {}",
        path.display()
      );
      path.canonicalize().with_context(|| {
        format!(
          "TERMINAI_PYTHON_DIR is set to '{}' but that path does not exist or is not accessible",
          env_dir
        )
      })?
    } else {
      // Auto-detect: try multiple strategies
      let exe_path = std::env::current_exe()?;
      let exe_dir = exe_path
        .parent()
        .context("Could not determine executable directory")?;

      log::debug!("Executable path: {}", exe_path.display());
      log::debug!("Executable directory: {}", exe_dir.display());
      log::debug!(
        "TERMINAI_PYTHON_DIR not set, trying auto-detection strategies"
      );

      // Strategy 1: Same directory as executable (some install layouts)
      let strategy1 = exe_dir.join("python");
      log::debug!(
        "Trying strategy 1 (same dir as exe): {}",
        strategy1.display()
      );

      if let Ok(canonical) = strategy1.canonicalize() {
        log::info!(
          "Found Python directory via strategy 1: {}",
          canonical.display()
        );
        canonical
      } else {
        log::debug!(
          "Strategy 1 failed: directory does not exist or is not accessible"
        );

        // Strategy 2: One level up from executable (standard install layout)
        let strategy2 = exe_dir.join("../python");
        log::debug!(
          "Trying strategy 2 (one level up): {}",
          strategy2.display()
        );

        if let Ok(canonical) = strategy2.canonicalize() {
          log::info!(
            "Found Python directory via strategy 2: {}",
            canonical.display()
          );
          canonical
        } else {
          log::debug!(
            "Strategy 2 failed: directory does not exist or is not accessible"
          );

          // Strategy 3: Current working directory (development)
          let strategy3 = std::env::current_dir()?.join("python");
          log::debug!(
            "Trying strategy 3 (current dir): {}",
            strategy3.display()
          );

          if let Ok(canonical) = strategy3.canonicalize() {
            log::info!(
              "Found Python directory via strategy 3: {}",
              canonical.display()
            );
            canonical
          } else {
            log::error!(
              "Strategy 3 failed: directory does not exist or is not accessible"
            );
            log::error!("All Python directory discovery strategies failed:");
            log::error!("  Strategy 1: {}", strategy1.display());
            log::error!("  Strategy 2: {}", strategy2.display());
            log::error!("  Strategy 3: {}", strategy3.display());
            log::error!("Current directory: {:?}", std::env::current_dir());
            log::error!("Executable: {}", exe_path.display());
            log::error!(
              "Hint: Package managers should set TERMINAI_PYTHON_DIR in wrapper scripts"
            );

            anyhow::bail!(
              "Could not find Python project directory. Tried:\n  1. {}\n  2. {}\n  3. {}\n\
              Executable: {}\nCurrent dir: {:?}\n\
              Hint: Set TERMINAI_PYTHON_DIR environment variable to specify the location",
              strategy1.display(),
              strategy2.display(),
              strategy3.display(),
              exe_path.display(),
              std::env::current_dir()
            );
          }
        }
      }
    };

    log::info!("Using Python project directory: {}", python_dir.display());

    // Spawn the subprocess
    // NOTE: The subprocess inherits ALL environment variables from the parent process,
    // including those loaded from ~/.config/terminai/terminai.env
    // This allows users to configure API keys and other settings via standard env vars
    log::info!("Spawning LLM agent subprocess");
    log::debug!("Python command: {}", config.python_command);
    log::debug!(
      "Port range: {}-{}",
      config.port_range.0,
      config.port_range.1
    );
    log::debug!("Host: {}", config.host);

    let mut command = Command::new("uv");
    command
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
      .kill_on_drop(true);

    // Add custom environment variables (useful for testing)
    for (key, value) in &config.env_vars {
      log::debug!("Setting env var: {}={}", key, value);
      command.env(key, value);
    }

    log::debug!("Spawning command: uv run python -m terminai_agent");
    log::debug!("Working directory: {}", python_dir.display());

    let mut child = command
      .spawn()
      .map_err(|e| {
        log::error!("Failed to spawn subprocess: {}", e);
        log::error!("Command: uv run python -m terminai_agent");
        log::error!("Working directory: {}", python_dir.display());
        log::error!("Make sure 'uv' is installed and in your PATH");
        log::error!(
          "Check that the Python project exists at: {}",
          python_dir.display()
        );
        e
      })
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
        log::debug!("[Python stdout] {}", line);
        if let Some(port_str) = line.strip_prefix("AG_UI_PORT=") {
          let port: u16 =
            port_str.parse().context("Failed to parse port number")?;
          log::info!("Subprocess reported port: {}", port);
          return Ok::<u16, anyhow::Error>(port);
        }
      }

      // Subprocess terminated - check if it's still running
      let status = child.try_wait()?;
      if let Some(exit_status) = status {
        // Process exited - wait a moment for stderr to be captured
        tokio::time::sleep(Duration::from_millis(100)).await;
        let stderr_output = stderr_lines.lock().await.join("\n");
        log::error!("Subprocess exited with status: {}", exit_status);
        if !stderr_output.is_empty() {
          log::error!("Subprocess stderr:\n{}", stderr_output);
        }
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

      log::error!("Subprocess terminated without providing port and without exit status");
      anyhow::bail!("Subprocess terminated without providing port")
    })
    .await
    .map_err(|_| {
      log::error!("Timeout (10s) waiting for subprocess to start");
      log::error!("The subprocess may have failed to start or is taking too long");
      log::error!("Check that:");
      log::error!("  1. uv is installed and in PATH");
      log::error!("  2. Python dependencies are installed at: {}", python_dir.display());
      log::error!("  3. terminai_agent module can be imported");
      anyhow::anyhow!("Timeout waiting for subprocess to start (waited 10 seconds)")
    })??;

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
