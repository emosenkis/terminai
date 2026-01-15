// TERMIN.AI: Tool call display and summarization
//
// This module provides human-friendly rendering of tool calls in the conversation UI.
// Each tool can register a summarizer function that formats its arguments and results
// into readable text.

use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tui::style::{Color, Modifier, Style};
use tui::text::{Line, Span};

/// Status of a tool call
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCallStatus {
  /// Tool is currently executing
  Running,
  /// Tool completed successfully
  Success,
  /// Tool execution failed
  Failed,
}

impl ToolCallStatus {
  /// Get the icon/emoji for this status
  pub fn icon(&self) -> &'static str {
    match self {
      ToolCallStatus::Running => "⏳",
      ToolCallStatus::Success => "✓",
      ToolCallStatus::Failed => "✗",
    }
  }

  /// Get the color for this status
  pub fn color(&self) -> Color {
    match self {
      ToolCallStatus::Running => Color::Yellow,
      ToolCallStatus::Success => Color::Green,
      ToolCallStatus::Failed => Color::Red,
    }
  }
}

/// A displayable tool call entry for the conversation UI
#[derive(Debug, Clone)]
pub struct ToolCallDisplay {
  /// Unique identifier for this tool call
  pub id: String,
  /// Name of the tool
  pub tool_name: String,
  /// Original arguments (for summarization)
  pub args: HashMap<String, JsonValue>,
  /// When the tool started executing
  pub started_at: Instant,
  /// How long the tool took to execute (set when complete)
  pub duration: Option<Duration>,
  /// Current status
  pub status: ToolCallStatus,
  /// Result content (if completed)
  pub result_content: Option<String>,
  /// Error message (if failed)
  pub error_message: Option<String>,
}

impl ToolCallDisplay {
  /// Create a new tool call display entry (starts in Running state)
  pub fn new(
    id: impl Into<String>,
    tool_name: impl Into<String>,
    args: HashMap<String, JsonValue>,
  ) -> Self {
    Self {
      id: id.into(),
      tool_name: tool_name.into(),
      args,
      started_at: Instant::now(),
      duration: None,
      status: ToolCallStatus::Running,
      result_content: None,
      error_message: None,
    }
  }

  /// Mark the tool call as completed successfully
  pub fn complete(&mut self, result_content: String) {
    self.duration = Some(self.started_at.elapsed());
    self.status = ToolCallStatus::Success;
    self.result_content = Some(result_content);
  }

  /// Mark the tool call as failed
  pub fn fail(&mut self, error_message: String) {
    self.duration = Some(self.started_at.elapsed());
    self.status = ToolCallStatus::Failed;
    self.error_message = Some(error_message);
  }

  /// Format the duration as a human-readable string
  pub fn format_duration(&self) -> String {
    match self.duration {
      Some(d) => {
        let millis = d.as_millis();
        if millis < 1000 {
          format!("{}ms", millis)
        } else {
          format!("{:.1}s", d.as_secs_f64())
        }
      }
      None => "...".to_string(),
    }
  }

  /// Render this tool call as Lines for the conversation UI
  pub fn render(&self) -> Vec<Line<'static>> {
    let summarizer = get_tool_summarizer(&self.tool_name);
    let summary = summarizer(&self.args, self.result_content.as_deref());

    let status_style = Style::default()
      .fg(self.status.color())
      .add_modifier(Modifier::BOLD);

    let duration_style = Style::default()
      .fg(Color::DarkGray)
      .add_modifier(Modifier::ITALIC);

    let tool_style = Style::default()
      .fg(Color::Magenta)
      .add_modifier(Modifier::BOLD);

    // Build the header line: [icon] tool_name (duration)
    // Clone strings to create owned data for 'static lifetime
    let header_spans = vec![
      Span::styled(format!("{} ", self.status.icon()), status_style),
      Span::styled(self.tool_name.clone(), tool_style),
      Span::styled(format!(" ({})", self.format_duration()), duration_style),
    ];

    let mut lines = vec![Line::from(header_spans)];

    // Add summary lines with indentation
    for summary_line in summary {
      lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(summary_line, Style::default().fg(Color::White)),
      ]));
    }

    // Add error message if failed
    if let Some(ref error) = self.error_message {
      lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(
          format!("Error: {}", error),
          Style::default().fg(Color::Red),
        ),
      ]));
    }

    // Add empty separator line
    lines.push(Line::from(""));

    lines
  }
}

/// Type alias for a tool summarizer function
///
/// Takes the tool arguments and optional result content,
/// returns a vector of summary lines to display.
pub type ToolSummarizer =
  fn(&HashMap<String, JsonValue>, Option<&str>) -> Vec<String>;

/// Get the summarizer for a given tool name
pub fn get_tool_summarizer(tool_name: &str) -> ToolSummarizer {
  match tool_name {
    "suggest_command" => summarize_suggest_command,
    "read_scrollback" => summarize_read_scrollback,
    _ => summarize_unknown_tool,
  }
}

/// Summarizer for the suggest_command tool
fn summarize_suggest_command(
  args: &HashMap<String, JsonValue>,
  _result: Option<&str>,
) -> Vec<String> {
  let mut lines = Vec::new();

  if let Some(JsonValue::String(cmd)) = args.get("command") {
    lines.push(format!("Command: {}", cmd));
  }

  if let Some(JsonValue::String(explanation)) = args.get("explanation") {
    lines.push(format!("Explanation: {}", explanation));
  }

  if lines.is_empty() {
    lines.push("Suggesting a command...".to_string());
  }

  lines
}

/// Summarizer for the read_scrollback tool
fn summarize_read_scrollback(
  args: &HashMap<String, JsonValue>,
  result: Option<&str>,
) -> Vec<String> {
  let mut lines = Vec::new();

  let num_lines = args
    .get("num_lines")
    .and_then(|v| v.as_i64())
    .unwrap_or(100);

  lines.push(format!("Reading {} lines of terminal history", num_lines));

  // Show how many lines were actually read
  if let Some(content) = result {
    let actual_lines = content.lines().count();
    lines.push(format!("Retrieved {} lines", actual_lines));
  }

  lines
}

/// Default summarizer for unknown tools
fn summarize_unknown_tool(
  args: &HashMap<String, JsonValue>,
  _result: Option<&str>,
) -> Vec<String> {
  let mut lines = Vec::new();

  if args.is_empty() {
    lines.push("(no arguments)".to_string());
  } else {
    for (key, value) in args {
      let value_str = match value {
        JsonValue::String(s) => {
          // Truncate long strings
          if s.len() > 50 {
            format!("\"{}...\"", &s[..47])
          } else {
            format!("\"{}\"", s)
          }
        }
        JsonValue::Number(n) => n.to_string(),
        JsonValue::Bool(b) => b.to_string(),
        JsonValue::Null => "null".to_string(),
        JsonValue::Array(arr) => format!("[{} items]", arr.len()),
        JsonValue::Object(obj) => format!("{{...{} keys}}", obj.len()),
      };
      lines.push(format!("{}: {}", key, value_str));
    }
  }

  lines
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_tool_call_display_lifecycle() {
    let mut args = HashMap::new();
    args.insert(
      "command".to_string(),
      JsonValue::String("ls -la".to_string()),
    );

    let mut display = ToolCallDisplay::new("test-id", "suggest_command", args);

    assert_eq!(display.status, ToolCallStatus::Running);
    assert!(display.duration.is_none());

    // Simulate some work
    std::thread::sleep(std::time::Duration::from_millis(10));

    display.complete("Success".to_string());

    assert_eq!(display.status, ToolCallStatus::Success);
    assert!(display.duration.is_some());
    assert!(display.duration.unwrap().as_millis() >= 10);
  }

  #[test]
  fn test_suggest_command_summarizer() {
    let mut args = HashMap::new();
    args.insert(
      "command".to_string(),
      JsonValue::String("git status".to_string()),
    );
    args.insert(
      "explanation".to_string(),
      JsonValue::String("Check repo status".to_string()),
    );

    let summary = summarize_suggest_command(&args, None);

    assert_eq!(summary.len(), 2);
    assert!(summary[0].contains("git status"));
    assert!(summary[1].contains("Check repo status"));
  }

  #[test]
  fn test_read_scrollback_summarizer() {
    let mut args = HashMap::new();
    args.insert("num_lines".to_string(), JsonValue::Number(50.into()));

    let result = "line1\nline2\nline3";
    let summary = summarize_read_scrollback(&args, Some(result));

    assert!(summary[0].contains("50 lines"));
    assert!(summary[1].contains("3 lines"));
  }

  #[test]
  fn test_unknown_tool_summarizer() {
    let mut args = HashMap::new();
    args.insert("foo".to_string(), JsonValue::String("bar".to_string()));
    args.insert("count".to_string(), JsonValue::Number(42.into()));

    let summary = summarize_unknown_tool(&args, None);

    assert_eq!(summary.len(), 2);
  }
}
