use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::{Arc, RwLock};

/// Maximum number of lines that can be read from scrollback
const MAX_SCROLLBACK_LINES: usize = 500;

#[derive(Deserialize)]
pub struct ReadScrollbackArgs {
  /// Number of lines to read from scrollback (max 500)
  lines: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum ReadScrollbackError {
  #[error("Failed to acquire scrollback lock")]
  LockError,
  #[error("Invalid number of lines requested: {0}")]
  InvalidLinesCount(usize),
}

/// Tool for reading additional shell scrollback history
pub struct ReadScrollbackTool {
  /// Shared scrollback buffer
  scrollback: Arc<RwLock<Vec<String>>>,
}

impl ReadScrollbackTool {
  pub fn new(scrollback: Arc<RwLock<Vec<String>>>) -> Self {
    Self { scrollback }
  }
}

impl Tool for ReadScrollbackTool {
  const NAME: &'static str = "read_scrollback";

  type Args = ReadScrollbackArgs;
  type Output = String;
  type Error = ReadScrollbackError;

  async fn definition(&self, _prompt: String) -> ToolDefinition {
    ToolDefinition {
      name: "read_scrollback".to_string(),
      description: "Read additional lines from the shell scrollback history. Use this to access more context about previous commands and their output.".to_string(),
      parameters: json!({
        "type": "object",
        "properties": {
          "lines": {
            "type": "integer",
            "description": format!("Number of lines to read from scrollback (1-{})", MAX_SCROLLBACK_LINES),
            "minimum": 1,
            "maximum": MAX_SCROLLBACK_LINES
          }
        },
        "required": ["lines"]
      }),
    }
  }

  async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
    // Validate line count
    if args.lines == 0 || args.lines > MAX_SCROLLBACK_LINES {
      return Err(ReadScrollbackError::InvalidLinesCount(args.lines));
    }

    // Read from scrollback
    let scrollback = self
      .scrollback
      .read()
      .map_err(|_| ReadScrollbackError::LockError)?;

    let lines_to_read = args.lines.min(scrollback.len());
    let start = scrollback.len().saturating_sub(lines_to_read);

    let lines = scrollback[start..]
      .iter()
      .map(|s| s.as_str())
      .collect::<Vec<_>>()
      .join("\n");

    Ok(if lines.is_empty() {
      "No additional scrollback available.".to_string()
    } else {
      format!(
        "## Scrollback (last {} lines)\n\n```\n{}\n```",
        lines_to_read, lines
      )
    })
  }
}
