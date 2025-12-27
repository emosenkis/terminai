use grep::matcher::Matcher;
use grep::regex::RegexMatcher;
use grep::searcher::{BinaryDetection, SearcherBuilder};
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use walkdir::WalkDir;

/// Maximum number of matches to return
const MAX_MATCHES: usize = 100;

/// Maximum number of files to search
const MAX_FILES: usize = 1000;

#[derive(Deserialize)]
pub struct GrepFilesArgs {
  /// Pattern to search for (regex or literal string)
  pattern: String,
  /// File glob pattern (e.g., "*.rs", "src/**/*.py")
  #[serde(default)]
  file_pattern: Option<String>,
  /// Whether to use case-insensitive search
  #[serde(default)]
  case_insensitive: bool,
  /// Maximum number of matches to return (default: 50, max: 100)
  #[serde(default)]
  max_matches: Option<usize>,
}

#[derive(Debug, thiserror::Error)]
pub enum GrepFilesError {
  #[error("Invalid regex pattern: {0}")]
  InvalidPattern(String),
  #[error("IO error: {0}")]
  IoError(#[from] std::io::Error),
  #[error("Walk directory error: {0}")]
  WalkDirError(#[from] walkdir::Error),
  #[error("Failed to acquire cwd lock")]
  LockError,
  #[error("Search error: {0}")]
  SearchError(String),
}

#[derive(Debug)]
struct Match {
  file: PathBuf,
  line_number: u64,
  line: String,
}

/// Tool for searching file contents using grep
pub struct GrepFilesTool {
  /// Current working directory
  cwd: Arc<RwLock<PathBuf>>,
}

impl GrepFilesTool {
  pub fn new(cwd: Arc<RwLock<PathBuf>>) -> Self {
    Self { cwd }
  }

  /// Search files for a pattern
  fn search_files(
    &self,
    cwd: &Path,
    pattern: &str,
    file_pattern: Option<&str>,
    case_insensitive: bool,
    max_matches: usize,
  ) -> Result<Vec<Match>, GrepFilesError> {
    // Build regex matcher
    let matcher = RegexMatcher::new_line_matcher(&pattern)
      .map_err(|e| GrepFilesError::InvalidPattern(e.to_string()))?;

    let mut matches = Vec::new();
    let mut files_searched = 0;

    // Walk the directory tree
    let walker = if let Some(pattern) = file_pattern {
      // TODO: Implement glob pattern filtering
      // For now, just walk all files
      WalkDir::new(cwd).max_depth(10).follow_links(false)
    } else {
      WalkDir::new(cwd).max_depth(10).follow_links(false)
    };

    for entry in walker {
      if matches.len() >= max_matches || files_searched >= MAX_FILES {
        break;
      }

      let entry = entry?;
      let path = entry.path();

      // Skip directories
      if !path.is_file() {
        continue;
      }

      // Skip binary files and large files
      if let Ok(metadata) = entry.metadata() {
        if metadata.len() > 10 * 1024 * 1024 {
          // Skip files > 10MB
          continue;
        }
      }

      files_searched += 1;

      // Search the file
      let mut searcher = SearcherBuilder::new()
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .line_number(true)
        .build();

      let path_buf = path.to_path_buf();
      let result = searcher.search_path(
        &matcher,
        path,
        grep::searcher::sinks::UTF8(|line_number, line| {
          if matches.len() < max_matches {
            matches.push(Match {
              file: path_buf.clone(),
              line_number,
              line: line.trim_end().to_string(),
            });
            Ok(true)
          } else {
            Ok(false)
          }
        }),
      );

      // Ignore errors for individual files (e.g., permission denied)
      let _ = result;
    }

    Ok(matches)
  }
}

impl Tool for GrepFilesTool {
  const NAME: &'static str = "grep_files";

  type Args = GrepFilesArgs;
  type Output = String;
  type Error = GrepFilesError;

  async fn definition(&self, _prompt: String) -> ToolDefinition {
    ToolDefinition {
      name: "grep_files".to_string(),
      description: "Search for a pattern in files under the current working directory. Returns matching lines with file paths and line numbers.".to_string(),
      parameters: json!({
        "type": "object",
        "properties": {
          "pattern": {
            "type": "string",
            "description": "Pattern to search for (supports regex)"
          },
          "file_pattern": {
            "type": "string",
            "description": "File glob pattern to filter files (e.g., '*.rs', '*.py'). Optional."
          },
          "case_insensitive": {
            "type": "boolean",
            "description": "Whether to perform case-insensitive search. Default is false.",
            "default": false
          },
          "max_matches": {
            "type": "integer",
            "description": format!("Maximum number of matches to return (default: 50, max: {})", MAX_MATCHES),
            "minimum": 1,
            "maximum": MAX_MATCHES,
            "default": 50
          }
        },
        "required": ["pattern"]
      }),
    }
  }

  async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
    let cwd = self.cwd.read().map_err(|_| GrepFilesError::LockError)?;

    let max_matches = args.max_matches.unwrap_or(50).min(MAX_MATCHES);

    // Search files
    let matches = self.search_files(
      &cwd,
      &args.pattern,
      args.file_pattern.as_deref(),
      args.case_insensitive,
      max_matches,
    )?;

    // Format results
    if matches.is_empty() {
      return Ok(format!("No matches found for pattern: {}", args.pattern));
    }

    let mut result = format!("## Grep Results for '{}'\n\n", args.pattern);
    result.push_str(&format!("Found {} matches:\n\n", matches.len()));

    let mut current_file: Option<PathBuf> = None;

    for m in &matches {
      // Show file path when it changes
      if current_file.as_ref() != Some(&m.file) {
        if current_file.is_some() {
          result.push_str("\n");
        }
        result.push_str(&format!("### {}\n\n", m.file.display()));
        current_file = Some(m.file.clone());
      }

      result.push_str(&format!("{}:  {}\n", m.line_number, m.line));
    }

    if matches.len() >= max_matches {
      result.push_str(&format!(
        "\n*Showing first {} matches. There may be more.*",
        max_matches
      ));
    }

    Ok(result)
  }
}
