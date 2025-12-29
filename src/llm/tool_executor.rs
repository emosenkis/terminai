// TERMIN.AI: Client-side tool execution for AG-UI tools
//
// This module handles executing tools (suggest_command, read_scrollback) on the Rust side
// when the LLM requests them via AG-UI protocol.

use ag_ui_core::types::ids::ToolCallId;
use anyhow::Result;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;
use tokio::sync::Mutex;

use crate::command::{CommandExecutor, RiskLevel, SafetyValidator};
use crate::shell::ReplySender;
use crate::vt100;

/// Request to execute a tool (sent from StreamingSubscriber to application layer)
#[derive(Debug, Clone)]
pub struct ToolExecutionRequest {
  pub tool_call_id: ToolCallId,
  pub tool_name: String,
  pub args: HashMap<String, JsonValue>,
}

/// Result of tool execution (to be sent back to LLM)
#[derive(Debug, Clone)]
pub struct ToolResult {
  pub tool_call_id: ToolCallId,
  pub content: String,
  pub is_error: bool,
}

/// A command suggestion from the LLM
#[derive(Debug, Clone)]
pub struct CommandSuggestion {
  pub command: String,
  pub explanation: Option<String>,
  pub risk_level: RiskLevel,
  pub timestamp: SystemTime,
}

/// Context needed for tool execution
pub struct ToolExecutionContext {
  /// Optional VT100 parser for reading scrollback
  pub vt_parser: Option<Arc<RwLock<vt100::Parser<ReplySender>>>>,
  /// Shared storage for command suggestions
  pub command_suggestions: Arc<Mutex<Vec<CommandSuggestion>>>,
  /// Command executor (reuse existing infrastructure)
  pub command_executor: CommandExecutor,
  /// Safety validator (reuse existing infrastructure)
  pub safety_validator: SafetyValidator,
}

/// Tool executor - handles execution of client-side tools
pub struct ToolExecutor {
  context: ToolExecutionContext,
}

impl ToolExecutor {
  pub fn new(context: ToolExecutionContext) -> Self {
    Self { context }
  }

  /// Execute a tool and return the result
  pub async fn execute_tool(
    &self,
    request: ToolExecutionRequest,
  ) -> ToolResult {
    log::info!(
      "Executing tool: {} (id: {:?})",
      request.tool_name,
      request.tool_call_id
    );

    match request.tool_name.as_str() {
      "suggest_command" => self.execute_suggest_command(request).await,
      "read_scrollback" => self.execute_read_scrollback(request).await,
      _ => ToolResult {
        tool_call_id: request.tool_call_id,
        content: format!("Unknown tool: {}", request.tool_name),
        is_error: true,
      },
    }
  }

  /// Execute suggest_command tool
  ///
  /// Stores the suggested command in shared storage for UI to display.
  /// Returns immediately without waiting for user approval.
  async fn execute_suggest_command(
    &self,
    request: ToolExecutionRequest,
  ) -> ToolResult {
    // Extract command and explanation from args
    let command = match request.args.get("command") {
      Some(JsonValue::String(cmd)) => cmd.clone(),
      _ => {
        return ToolResult {
          tool_call_id: request.tool_call_id,
          content: "Missing or invalid 'command' parameter".to_string(),
          is_error: true,
        };
      }
    };

    let explanation = request
      .args
      .get("explanation")
      .and_then(|v| v.as_str())
      .map(|s| s.to_string());

    // Assess risk level using existing SafetyValidator
    let risk_level = self.context.safety_validator.assess_risk(&command);

    // Store suggestion for UI to display
    let suggestion = CommandSuggestion {
      command: command.clone(),
      explanation: explanation.clone(),
      risk_level,
      timestamp: SystemTime::now(),
    };

    {
      let mut suggestions = self.context.command_suggestions.lock().await;
      suggestions.push(suggestion);
    }

    log::info!(
      "Stored command suggestion: {} (risk: {:?})",
      command,
      risk_level
    );

    // Return success immediately - don't wait for user to execute
    let explanation_text =
      explanation.map(|e| format!(" - {}", e)).unwrap_or_default();

    ToolResult {
      tool_call_id: request.tool_call_id,
      content: format!(
        "Command suggestion stored: {}{}",
        command, explanation_text
      ),
      is_error: false,
    }
  }

  /// Execute read_scrollback tool
  ///
  /// Reads N lines from terminal scrollback and returns as text.
  async fn execute_read_scrollback(
    &self,
    request: ToolExecutionRequest,
  ) -> ToolResult {
    // Extract num_lines parameter (default: 100)
    let num_lines = request
      .args
      .get("num_lines")
      .and_then(|v| v.as_i64())
      .unwrap_or(100) as usize;

    log::info!("Reading {} lines from scrollback", num_lines);

    // Access VT100 parser if available
    let vt_parser = match &self.context.vt_parser {
      Some(parser) => parser,
      None => {
        return ToolResult {
          tool_call_id: request.tool_call_id,
          content: "VT100 parser not available".to_string(),
          is_error: true,
        };
      }
    };

    // Extract scrollback text
    match self.extract_scrollback(vt_parser, num_lines) {
      Ok(text) => {
        log::debug!(
          "Successfully extracted {} characters of scrollback",
          text.len()
        );
        ToolResult {
          tool_call_id: request.tool_call_id,
          content: text,
          is_error: false,
        }
      }
      Err(e) => {
        log::error!("Failed to extract scrollback: {}", e);
        ToolResult {
          tool_call_id: request.tool_call_id,
          content: format!("Failed to read scrollback: {}", e),
          is_error: true,
        }
      }
    }
  }

  /// Extract scrollback text from VT100 screen
  fn extract_scrollback(
    &self,
    vt_parser: &Arc<RwLock<vt100::Parser<ReplySender>>>,
    num_lines: usize,
  ) -> Result<String> {
    let parser = vt_parser
      .read()
      .map_err(|e| anyhow::anyhow!("Failed to lock VT parser: {}", e))?;
    let screen = parser.screen();

    // Collect all rows
    let all_rows: Vec<_> = screen.all_rows().collect();

    // Take last N lines
    let start_idx = all_rows.len().saturating_sub(num_lines);
    let rows_to_extract = &all_rows[start_idx..];

    // Extract text from each row
    let mut lines = Vec::new();
    for row in rows_to_extract {
      let mut line = String::new();

      // Extract cell contents
      for col_idx in 0..screen.size().cols {
        if let Some(cell) = row.get(col_idx) {
          if cell.has_contents() {
            line.push_str(&cell.contents());
          }
        }
      }

      // Only include non-empty lines
      let trimmed = line.trim_end();
      if !trimmed.is_empty() {
        lines.push(trimmed.to_string());
      }
    }

    Ok(lines.join("\n"))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn test_executor_creation() {
    let context = ToolExecutionContext {
      vt_parser: None,
      command_suggestions: Arc::new(Mutex::new(Vec::new())),
      command_executor: CommandExecutor::new(),
      safety_validator: SafetyValidator::new(),
    };

    let _executor = ToolExecutor::new(context);
  }

  #[tokio::test]
  async fn test_suggest_command_stores_suggestion() {
    let suggestions = Arc::new(Mutex::new(Vec::new()));

    let context = ToolExecutionContext {
      vt_parser: None,
      command_suggestions: Arc::clone(&suggestions),
      command_executor: CommandExecutor::new(),
      safety_validator: SafetyValidator::new(),
    };

    let executor = ToolExecutor::new(context);

    let mut args = HashMap::new();
    args.insert(
      "command".to_string(),
      JsonValue::String("ls -la".to_string()),
    );
    args.insert(
      "explanation".to_string(),
      JsonValue::String("List all files".to_string()),
    );

    let request = ToolExecutionRequest {
      tool_call_id: ToolCallId::random(),
      tool_name: "suggest_command".to_string(),
      args,
    };

    let result = executor.execute_tool(request).await;

    assert!(!result.is_error);
    assert!(result.content.contains("ls -la"));

    // Verify suggestion was stored
    let stored = suggestions.lock().await;
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].command, "ls -la");
    assert_eq!(stored[0].explanation, Some("List all files".to_string()));
    assert_eq!(stored[0].risk_level, RiskLevel::Safe);
  }

  #[tokio::test]
  async fn test_suggest_command_assesses_risk() {
    let suggestions = Arc::new(Mutex::new(Vec::new()));

    let context = ToolExecutionContext {
      vt_parser: None,
      command_suggestions: Arc::clone(&suggestions),
      command_executor: CommandExecutor::new(),
      safety_validator: SafetyValidator::new(),
    };

    let executor = ToolExecutor::new(context);

    // Test dangerous command
    let mut args = HashMap::new();
    args.insert(
      "command".to_string(),
      JsonValue::String("rm -rf /".to_string()),
    );

    let request = ToolExecutionRequest {
      tool_call_id: ToolCallId::random(),
      tool_name: "suggest_command".to_string(),
      args,
    };

    let _result = executor.execute_tool(request).await;

    // Verify dangerous command was assessed correctly
    let stored = suggestions.lock().await;
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].risk_level, RiskLevel::Dangerous);
  }

  #[tokio::test]
  async fn test_read_scrollback_requires_vt_parser() {
    let context = ToolExecutionContext {
      vt_parser: None, // No parser available
      command_suggestions: Arc::new(Mutex::new(Vec::new())),
      command_executor: CommandExecutor::new(),
      safety_validator: SafetyValidator::new(),
    };

    let executor = ToolExecutor::new(context);

    let request = ToolExecutionRequest {
      tool_call_id: ToolCallId::random(),
      tool_name: "read_scrollback".to_string(),
      args: HashMap::new(),
    };

    let result = executor.execute_tool(request).await;

    assert!(result.is_error);
    assert!(result.content.contains("not available"));
  }

  #[tokio::test]
  async fn test_unknown_tool() {
    let context = ToolExecutionContext {
      vt_parser: None,
      command_suggestions: Arc::new(Mutex::new(Vec::new())),
      command_executor: CommandExecutor::new(),
      safety_validator: SafetyValidator::new(),
    };

    let executor = ToolExecutor::new(context);

    let request = ToolExecutionRequest {
      tool_call_id: ToolCallId::random(),
      tool_name: "unknown_tool".to_string(),
      args: HashMap::new(),
    };

    let result = executor.execute_tool(request).await;

    assert!(result.is_error);
    assert!(result.content.contains("Unknown tool"));
  }
}
