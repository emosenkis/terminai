use std::sync::{Arc, RwLock};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use tokio::sync::{Mutex, mpsc};

use crate::agent_tools::PendingCommand;
use crate::command::SafetyValidator;
use crate::privacy::PrivacyFilter;
use crate::shell::ReplySender;
use crate::vt100;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolResponse {
  pub text: String,
  pub data: JsonValue,
}

#[derive(Clone)]
pub struct TerminaiMcpState {
  vt_parser: Arc<RwLock<vt100::Parser<ReplySender>>>,
  suggestions: mpsc::UnboundedSender<PendingCommand>,
  privacy_filter: Arc<PrivacyFilter>,
  safety_validator: Arc<SafetyValidator>,
  last_suggestion: Arc<Mutex<Option<PendingCommand>>>,
}

impl TerminaiMcpState {
  pub fn new(
    vt_parser: Arc<RwLock<vt100::Parser<ReplySender>>>,
    suggestions: mpsc::UnboundedSender<PendingCommand>,
  ) -> Self {
    Self {
      vt_parser,
      suggestions,
      privacy_filter: Arc::new(PrivacyFilter::new()),
      safety_validator: Arc::new(SafetyValidator::new()),
      last_suggestion: Arc::new(Mutex::new(None)),
    }
  }

  pub fn tool_definitions() -> JsonValue {
    json!([
      {
        "name": "read_terminal",
        "description": "Read the user's wrapped terminal screen and recent scrollback. Use this before answering questions about what is happening in the terminal.",
        "inputSchema": {
          "type": "object",
          "properties": {
            "max_lines": {
              "type": "integer",
              "description": "Maximum number of terminal lines to return.",
              "default": 120
            },
            "include_visible": {
              "type": "boolean",
              "description": "Include the visible terminal screen as well as scrollback.",
              "default": true
            }
          }
        }
      },
      {
        "name": "get_terminal_context",
        "description": "Get concise metadata about the wrapped terminal: cwd, shell, OS, size, mouse mode, and bracketed paste state.",
        "inputSchema": {
          "type": "object",
          "properties": {}
        }
      },
      {
        "name": "suggest_input",
        "description": "Suggest exact input for Termin.AI to offer to the user for approval before sending it to the wrapped shell. Do not use this for input to your own AI terminal.",
        "inputSchema": {
          "type": "object",
          "properties": {
            "input": {
              "type": "string",
              "description": "Exact terminal input to send after approval. Use escape sequences such as \\r for Enter, \\u0003 for Ctrl-C, and \\u001b for Escape."
            },
            "explanation": {
              "type": "string",
              "description": "Brief explanation shown to the user."
            }
          },
          "required": ["input"]
        }
      },
      {
        "name": "get_suggestion_status",
        "description": "Return the most recent shell input suggestion queued through suggest_input.",
        "inputSchema": {
          "type": "object",
          "properties": {}
        }
      }
    ])
  }

  pub async fn call_tool(
    &self,
    name: &str,
    args: JsonValue,
  ) -> Result<McpToolResponse> {
    match name {
      "read_terminal" => self.read_terminal(args).await,
      "get_terminal_context" => self.get_terminal_context().await,
      "suggest_input" => self.suggest_input(args).await,
      "get_suggestion_status" => self.get_suggestion_status().await,
      _ => anyhow::bail!("Unknown Termin.AI MCP tool: {}", name),
    }
  }

  async fn read_terminal(&self, args: JsonValue) -> Result<McpToolResponse> {
    let max_lines = args
      .get("max_lines")
      .and_then(|v| v.as_u64())
      .unwrap_or(120)
      .max(1) as usize;

    let parser = self
      .vt_parser
      .read()
      .map_err(|e| anyhow::anyhow!("failed to lock terminal parser: {}", e))?;
    let screen = parser.screen();
    let rows: Vec<_> = screen.all_rows().collect();
    let start = rows.len().saturating_sub(max_lines);
    let mut lines = Vec::new();

    for row in &rows[start..] {
      let mut line = String::new();
      let mut has_content = false;
      for col in 0..screen.size().cols {
        if let Some(cell) = row.get(col) {
          if cell.has_contents() {
            line.push_str(cell.contents());
            has_content = true;
          } else if has_content {
            line.push(' ');
          }
        }
      }
      let trimmed = line.trim_end();
      if !trimmed.is_empty() {
        lines.push(trimmed.to_string());
      }
    }

    let filtered_lines = self.privacy_filter.filter_lines(&lines);
    let text = filtered_lines.join("\n");

    Ok(McpToolResponse {
      text: if text.is_empty() {
        "The wrapped terminal currently has no readable text.".to_string()
      } else {
        text.clone()
      },
      data: json!({
        "lines": filtered_lines,
        "line_count": lines.len(),
        "total_rows": screen.total_rows(),
        "visible_rows": screen.size().rows,
        "visible_cols": screen.size().cols
      }),
    })
  }

  async fn get_terminal_context(&self) -> Result<McpToolResponse> {
    let parser = self
      .vt_parser
      .read()
      .map_err(|e| anyhow::anyhow!("failed to lock terminal parser: {}", e))?;
    let screen = parser.screen();
    let cwd = std::env::current_dir()
      .unwrap_or_else(|_| std::path::PathBuf::from("/"))
      .display()
      .to_string();
    let shell = std::env::var("SHELL").ok();
    let data = json!({
      "cwd": cwd,
      "shell": shell,
      "os": std::env::consts::OS,
      "terminal": {
        "rows": screen.size().rows,
        "cols": screen.size().cols,
        "mouse_protocol": format!("{:?}", screen.mouse_protocol_mode()),
        "bracketed_paste": screen.bracketed_paste()
      }
    });

    Ok(McpToolResponse {
      text: format!("cwd: {}\nos: {}", data["cwd"], std::env::consts::OS),
      data,
    })
  }

  async fn suggest_input(&self, args: JsonValue) -> Result<McpToolResponse> {
    let command = args
      .get("input")
      .or_else(|| args.get("command"))
      .and_then(|v| v.as_str())
      .ok_or_else(|| anyhow::anyhow!("suggest_input requires input"))?
      .to_string();
    let explanation = args
      .get("explanation")
      .and_then(|v| v.as_str())
      .map(ToString::to_string);
    let risk_level = self.safety_validator.assess_risk(&command);
    let pending =
      PendingCommand::new(command.clone(), explanation.clone(), risk_level);

    {
      let mut last = self.last_suggestion.lock().await;
      *last = Some(pending.clone());
    }
    self.suggestions.send(pending)?;

    Ok(McpToolResponse {
      text: format!(
        "Queued shell input suggestion for user approval: {}",
        command
      ),
      data: json!({
        "queued": true,
        "input": command,
        "explanation": explanation,
        "risk_level": format!("{:?}", risk_level)
      }),
    })
  }

  async fn get_suggestion_status(&self) -> Result<McpToolResponse> {
    let suggestion = self.last_suggestion.lock().await.clone();
    let data = match suggestion {
      Some(pending) => json!({
        "has_suggestion": true,
        "input": pending.command,
        "explanation": pending.explanation,
        "risk_level": format!("{:?}", pending.risk_level)
      }),
      None => json!({ "has_suggestion": false }),
    };

    Ok(McpToolResponse {
      text: data.to_string(),
      data,
    })
  }
}
