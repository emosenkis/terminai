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
}

impl TerminalContext {
  /// Convert to AG-UI Context items
  pub fn to_ag_ui_context(&self) -> Vec<Context> {
    let mut context_items = Vec::new();

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
