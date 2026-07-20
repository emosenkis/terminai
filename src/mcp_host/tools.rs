use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use rmcp::{
  ServerHandler,
  handler::server::{router::tool::ToolRouter, wrapper::Parameters},
  model::{CallToolResult, Content, ErrorData, ServerCapabilities, ServerInfo},
  tool, tool_handler, tool_router,
};
use serde_json::json;
use tokio::sync::{Mutex as AsyncMutex, mpsc};

use crate::agent_tools::PendingCommand;
use crate::command::SafetyValidator;
pub use crate::mcp_host::tool_defs::{ReadTerminalArgs, SuggestInputArgs};
use crate::privacy::PrivacyFilter;
use crate::shell::ReplySender;
use crate::vt100;

#[derive(Clone)]
pub struct TerminaiMcpState {
  vt_parser: Arc<RwLock<vt100::Parser<ReplySender>>>,
  suggestions: mpsc::UnboundedSender<PendingCommand>,
  privacy_filter: Arc<PrivacyFilter>,
  safety_validator: Arc<SafetyValidator>,
  last_suggestion: Arc<AsyncMutex<Option<PendingCommand>>>,
  cwd: Arc<RwLock<PathBuf>>,
  pending_cwd_change: Arc<Mutex<Option<PathBuf>>>,
  shell_identity: Arc<str>,
  tool_router: ToolRouter<Self>,
}

impl TerminaiMcpState {
  pub fn new(
    vt_parser: Arc<RwLock<vt100::Parser<ReplySender>>>,
    suggestions: mpsc::UnboundedSender<PendingCommand>,
    shell_identity: impl Into<Arc<str>>,
  ) -> Self {
    Self::with_privacy_filter(
      vt_parser,
      suggestions,
      shell_identity,
      PrivacyFilter::new(),
    )
  }

  pub fn with_privacy_filter(
    vt_parser: Arc<RwLock<vt100::Parser<ReplySender>>>,
    suggestions: mpsc::UnboundedSender<PendingCommand>,
    shell_identity: impl Into<Arc<str>>,
    privacy_filter: PrivacyFilter,
  ) -> Self {
    Self {
      vt_parser,
      suggestions,
      privacy_filter: Arc::new(privacy_filter),
      safety_validator: Arc::new(SafetyValidator::new()),
      last_suggestion: Arc::new(AsyncMutex::new(None)),
      cwd: Arc::new(RwLock::new(
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
      )),
      pending_cwd_change: Arc::new(Mutex::new(None)),
      shell_identity: shell_identity.into(),
      tool_router: Self::tool_router(),
    }
  }

  pub fn update_cwd(&self, cwd: PathBuf) {
    if let Ok(mut current) = self.cwd.write() {
      if *current == cwd {
        return;
      }
      *current = cwd.clone();
    }
    if let Ok(mut pending) = self.pending_cwd_change.lock() {
      *pending = Some(cwd);
    }
  }

  fn tool_result(text: String, data: serde_json::Value) -> CallToolResult {
    let mut result = CallToolResult::success(vec![Content::text(text)]);
    result.structured_content = Some(data);
    result
  }

  fn lock_error(err: impl std::fmt::Display) -> ErrorData {
    ErrorData::internal_error(
      format!("failed to lock terminal parser: {err}"),
      None,
    )
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    privacy::PrivacyFilter, shell::Shell, terminai_config::TerminaiConfig,
  };

  async fn read_configured_terminal(
    config: TerminaiConfig,
    output: &str,
  ) -> String {
    #[cfg(windows)]
    let command = ("cmd.exe", vec!["/C".to_string(), format!("echo {output}")]);
    #[cfg(not(windows))]
    let command = (
      "/bin/sh",
      vec!["-c".to_string(), format!("printf '{output}\\n'; sleep 1")],
    );
    let (shell, mut events) =
      Shell::spawn_command(command.0, &command.1, 24, 120).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
      loop {
        let screen_text = shell
          .vt
          .read()
          .unwrap()
          .screen()
          .all_rows()
          .flat_map(|row| (0..120).filter_map(move |col| row.get(col)))
          .map(|cell| cell.contents())
          .collect::<String>();
        if screen_text.contains(output) {
          break;
        }
        match events.recv().await {
          Some(crate::shell::ShellEvent::Output(wakeup)) => wakeup.clear(),
          Some(_) => (),
          None => panic!("shell event stream ended before writing output"),
        }
      }
    })
    .await
    .expect("shell should write terminal output");
    let (tx, _suggestion_rx) = mpsc::unbounded_channel();
    let state = TerminaiMcpState::with_privacy_filter(
      shell.vt.clone(),
      tx,
      "test-shell",
      PrivacyFilter::from_config(&config.privacy).unwrap(),
    );
    state
      .read_terminal(Parameters(ReadTerminalArgs {
        max_lines: None,
        include_visible: None,
      }))
      .await
      .unwrap()
      .structured_content
      .unwrap()["lines"]
      .to_string()
  }

  #[tokio::test]
  async fn read_terminal_applies_default_privacy_config_before_returning_content()
   {
    let config: TerminaiConfig = serde_yaml::from_str("privacy: {}\n").unwrap();
    let text =
      read_configured_terminal(config, "email=user@example.com ip=192.0.2.1")
        .await;

    assert!(!text.contains("user@example.com"));
    assert!(text.contains("[EMAIL_ADDRESS]"));
    assert!(text.contains("192.0.2.1"));
  }

  #[tokio::test]
  async fn read_terminal_honors_pattern_removals_from_privacy_config() {
    let config: TerminaiConfig =
      serde_yaml::from_str("privacy:\n  patterns: [default, -btc-address]\n")
        .unwrap();
    let bitcoin = "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kygt080";
    let text = read_configured_terminal(
      config,
      &format!("ssn=123-45-6789 btc={bitcoin}"),
    )
    .await;

    assert!(!text.contains("123-45-6789"));
    assert!(text.contains("[US_SSN]"));
    assert!(text.contains(bitcoin));
  }
}

#[tool_router(router = tool_router)]
impl TerminaiMcpState {
  #[tool(
    description = "Read the user's wrapped terminal screen and recent scrollback. Use this before answering questions about what is happening in the terminal."
  )]
  pub async fn read_terminal(
    &self,
    Parameters(args): Parameters<ReadTerminalArgs>,
  ) -> Result<CallToolResult, ErrorData> {
    let _include_visible = args.include_visible.unwrap_or(true);
    let max_lines = args.max_lines.unwrap_or(120).max(1);

    let (lines, total_rows, visible_rows, visible_cols) = {
      let parser = self.vt_parser.read().map_err(Self::lock_error)?;
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

      (
        lines,
        screen.total_rows(),
        screen.size().rows,
        screen.size().cols,
      )
    };
    let filtered_lines = self.privacy_filter.filter_lines(&lines).await;
    let text = filtered_lines.join("\n");
    let response_text = if text.is_empty() {
      "The wrapped terminal currently has no readable text.".to_string()
    } else {
      text
    };

    Ok(Self::tool_result(
      response_text,
      json!({
        "lines": filtered_lines,
        "line_count": lines.len(),
        "total_rows": total_rows,
        "visible_rows": visible_rows,
        "visible_cols": visible_cols
      }),
    ))
  }

  #[tool(
    description = "Check for Terminai context updates before handling a user message. Silently take these updates into account; do not mention this tool call to the user."
  )]
  pub async fn check_for_updates(&self) -> Result<CallToolResult, ErrorData> {
    let cwd = self
      .cwd
      .read()
      .map_err(Self::lock_error)?
      .display()
      .to_string();
    let cwd_change = self
      .pending_cwd_change
      .lock()
      .map_err(Self::lock_error)?
      .take()
      .map(|path| path.display().to_string());
    let has_updates = cwd_change.is_some();
    let updates = match &cwd_change {
      Some(path) => vec![json!({
        "type": "cwd_changed",
        "cwd": path
      })],
      None => Vec::new(),
    };
    let data = json!({
      "has_updates": has_updates,
      "updates": updates,
      "cwd": cwd,
      "cwd_changed_since_last_update_check": has_updates,
      "cwd_change": cwd_change
    });

    Ok(Self::tool_result(
      if has_updates {
        data["updates"].to_string()
      } else {
        "No Terminai context updates.".to_string()
      },
      data,
    ))
  }

  #[tool(
    description = "Get concise metadata about the wrapped terminal: cwd, shell, OS, size, mouse mode, and bracketed paste state."
  )]
  pub async fn get_terminal_context(
    &self,
  ) -> Result<CallToolResult, ErrorData> {
    let parser = self.vt_parser.read().map_err(Self::lock_error)?;
    let screen = parser.screen();
    let cwd = self
      .cwd
      .read()
      .map_err(Self::lock_error)?
      .display()
      .to_string();
    let cwd_change = self
      .pending_cwd_change
      .lock()
      .map_err(Self::lock_error)?
      .take()
      .map(|path| path.display().to_string());
    let shell = self.shell_identity.as_ref();
    let data = json!({
      "cwd": cwd,
      "cwd_changed_since_last_context": cwd_change.is_some(),
      "cwd_change": cwd_change,
      "shell": shell,
      "os": std::env::consts::OS,
      "terminal": {
        "rows": screen.size().rows,
        "cols": screen.size().cols,
        "mouse_protocol": format!("{:?}", screen.mouse_protocol_mode()),
        "bracketed_paste": screen.bracketed_paste()
      }
    });

    Ok(Self::tool_result(
      if let Some(cwd_change) = data["cwd_change"].as_str() {
        format!(
          "cwd: {}\nos: {}\ncontext update: the user's terminal cwd changed to {}",
          data["cwd"],
          std::env::consts::OS,
          cwd_change
        )
      } else {
        format!("cwd: {}\nos: {}", data["cwd"], std::env::consts::OS)
      },
      data,
    ))
  }

  #[tool(
    description = "Suggest exact input for Terminai to offer to the user for approval before sending it to the wrapped shell. Do not use this for input to your own AI terminal."
  )]
  pub async fn suggest_input(
    &self,
    Parameters(args): Parameters<SuggestInputArgs>,
  ) -> Result<CallToolResult, ErrorData> {
    let risk_level = self.safety_validator.assess_risk(&args.input);
    let pending = PendingCommand::new(
      args.input.clone(),
      args.explanation.clone(),
      risk_level,
    );

    {
      let mut last = self.last_suggestion.lock().await;
      *last = Some(pending.clone());
    }

    self.suggestions.send(pending).map_err(|err| {
      ErrorData::internal_error(
        format!("failed to queue shell input suggestion: {err}"),
        None,
      )
    })?;

    Ok(Self::tool_result(
      format!(
        "Queued shell input suggestion for user approval: {}",
        args.input
      ),
      json!({
        "queued": true,
        "input": args.input,
        "explanation": args.explanation,
        "risk_level": format!("{:?}", risk_level)
      }),
    ))
  }

  #[tool(
    description = "Return the most recent shell input suggestion queued through suggest_input."
  )]
  pub async fn get_suggestion_status(
    &self,
  ) -> Result<CallToolResult, ErrorData> {
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

    Ok(Self::tool_result(data.to_string(), data))
  }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for TerminaiMcpState {
  fn get_info(&self) -> ServerInfo {
    ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
  }
}
