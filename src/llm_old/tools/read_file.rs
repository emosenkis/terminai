use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// Maximum number of lines that can be read from a file
const MAX_FILE_LINES: usize = 1000;

#[derive(Deserialize)]
pub struct ReadFileArgs {
  /// Path to the file to read (relative to cwd or absolute)
  path: String,
  /// Starting line number (0-indexed, optional)
  #[serde(default)]
  start_line: Option<usize>,
  /// Number of lines to read (optional, default: 100)
  #[serde(default)]
  max_lines: Option<usize>,
}

#[derive(Debug, thiserror::Error)]
pub enum ReadFileError {
  #[error("File not found: {0}")]
  FileNotFound(String),
  #[error("IO error: {0}")]
  IoError(#[from] std::io::Error),
  #[error("Invalid line range")]
  InvalidLineRange,
  #[error("Failed to acquire cwd lock")]
  LockError,
  #[error("Path traversal detected: {0}")]
  PathTraversal(String),
}

/// Tool for reading files by path
pub struct ReadFileTool {
  /// Current working directory
  cwd: Arc<RwLock<PathBuf>>,
}

impl ReadFileTool {
  pub fn new(cwd: Arc<RwLock<PathBuf>>) -> Self {
    Self { cwd }
  }

  /// Check if a path is safe (no path traversal outside cwd)
  fn is_safe_path(&self, path: &Path, cwd: &Path) -> bool {
    // Resolve the path and check if it's within cwd
    if let Ok(canonical) = path.canonicalize() {
      if let Ok(canonical_cwd) = cwd.canonicalize() {
        return canonical.starts_with(canonical_cwd);
      }
    }

    // If canonicalize fails (file doesn't exist yet), check components
    let resolved = if path.is_absolute() {
      path.to_path_buf()
    } else {
      cwd.join(path)
    };

    // Check for .. components that might escape cwd
    !resolved
      .components()
      .any(|c| matches!(c, std::path::Component::ParentDir))
  }
}

impl Tool for ReadFileTool {
  const NAME: &'static str = "read_file";

  type Args = ReadFileArgs;
  type Output = String;
  type Error = ReadFileError;

  async fn definition(&self, _prompt: String) -> ToolDefinition {
    ToolDefinition {
      name: "read_file".to_string(),
      description: "Read contents of a file by path. Supports reading specific line ranges to avoid overwhelming the context.".to_string(),
      parameters: json!({
        "type": "object",
        "properties": {
          "path": {
            "type": "string",
            "description": "Path to the file to read (relative to current working directory or absolute)"
          },
          "start_line": {
            "type": "integer",
            "description": "Starting line number (0-indexed, optional)",
            "minimum": 0
          },
          "max_lines": {
            "type": "integer",
            "description": format!("Maximum number of lines to read (default: 100, max: {})", MAX_FILE_LINES),
            "minimum": 1,
            "maximum": MAX_FILE_LINES
          }
        },
        "required": ["path"]
      }),
    }
  }

  async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
    let cwd = self.cwd.read().map_err(|_| ReadFileError::LockError)?;

    // Resolve the path
    let path = Path::new(&args.path);
    let full_path = if path.is_absolute() {
      path.to_path_buf()
    } else {
      cwd.join(path)
    };

    // Security check: prevent path traversal
    if !self.is_safe_path(&full_path, &cwd) {
      return Err(ReadFileError::PathTraversal(args.path));
    }

    // Check if file exists
    if !full_path.exists() {
      return Err(ReadFileError::FileNotFound(args.path));
    }

    // Read the file
    let content = std::fs::read_to_string(&full_path)?;
    let lines: Vec<&str> = content.lines().collect();

    // Apply line range
    let start_line = args.start_line.unwrap_or(0);
    let max_lines = args.max_lines.unwrap_or(100).min(MAX_FILE_LINES);

    if start_line >= lines.len() {
      return Err(ReadFileError::InvalidLineRange);
    }

    let end_line = (start_line + max_lines).min(lines.len());
    let selected_lines = &lines[start_line..end_line];

    let result = selected_lines.join("\n");
    let total_lines = lines.len();

    Ok(format!(
      "## File: {}\n\nShowing lines {}-{} of {} total lines:\n\n```\n{}\n```",
      args.path,
      start_line + 1,
      end_line,
      total_lines,
      result
    ))
  }
}
