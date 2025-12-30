// TERMIN.AI: Terminal context conversion for AG-UI

use ag_ui_core::types::context::Context;

/// Terminal context information for the AI agent
#[derive(Debug, Clone)]
pub struct TerminalContext {
  /// Recent terminal history lines
  pub history_lines: Vec<String>,
  /// Current working directory
  pub cwd: String,
  /// Last command exit code
  pub last_exit_code: Option<i32>,
  /// Operating system information
  pub os_info: Option<String>,
  /// User's shell
  pub shell: Option<String>,
}

impl TerminalContext {
  /// Get the operating system information
  pub fn get_os_info() -> String {
    std::env::consts::OS.to_string()
  }

  /// Get the user's shell from SHELL environment variable
  pub fn get_shell() -> Option<String> {
    std::env::var("SHELL").ok().and_then(|path| {
      std::path::Path::new(&path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|s| s.to_string())
    })
  }

  /// Convert to AG-UI Context items
  pub fn to_ag_ui_context(&self) -> Vec<Context> {
    let mut context_items = Vec::new();

    // Add operating system info if available
    if let Some(os) = &self.os_info {
      context_items.push(Context {
        description: "Operating system".to_string(),
        value: os.clone(),
      });
    }

    // Add shell info if available
    if let Some(shell) = &self.shell {
      context_items.push(Context {
        description: "Shell".to_string(),
        value: shell.clone(),
      });
    }

    // Add terminal history as context
    if !self.history_lines.is_empty() {
      context_items.push(Context {
        description: "Recent terminal history".to_string(),
        value: self.history_lines.join("\n"),
      });
    }

    // Add current working directory
    context_items.push(Context {
      description: "Current working directory".to_string(),
      value: self.cwd.clone(),
    });

    // Add last exit code if available
    if let Some(code) = self.last_exit_code {
      context_items.push(Context {
        description: "Last command exit code".to_string(),
        value: code.to_string(),
      });
    }

    context_items
  }
}
