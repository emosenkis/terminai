use std::path::PathBuf;

use crate::{llm::AgUiTerminalContext, proc::view::ProcView};

/// Extract terminal context from process views
pub struct ContextExtractor {
  max_history_lines: usize,
}

impl ContextExtractor {
  pub fn new(max_history_lines: usize) -> Self {
    Self { max_history_lines }
  }

  /// Extract context from terminal output
  pub fn extract_context(
    &self,
    proc_views: &[ProcView],
    process_id: Option<usize>,
    cwd: PathBuf,
  ) -> AgUiTerminalContext {
    // Get the target process, defaulting to the first one if not specified
    let target_proc = if let Some(id) = process_id {
      proc_views.get(id)
    } else {
      proc_views.first()
    };

    let mut history_lines = Vec::new();
    let mut exit_code = None;

    if let Some(proc) = target_proc {
      // Extract exit code
      exit_code = proc.exit_code().map(|code| code as i32);

      // Extract terminal buffer lines
      if let Some(vt) = &proc.vt {
        if let Ok(parser) = vt.read() {
          let screen = parser.screen();
          let size = screen.size();

          // Calculate how many rows to extract (up to max_history_lines)
          let rows_to_extract = self.max_history_lines.min(size.rows as usize);

          // Extract text row by row using the public cell() API
          for row_idx in 0..rows_to_extract {
            let mut line_content = String::new();
            let mut has_content = false;

            // Extract each cell in the row
            for col_idx in 0..size.cols {
              if let Some(cell) = screen.cell(row_idx as u16, col_idx) {
                if cell.has_contents() {
                  line_content.push_str(&cell.contents());
                  has_content = true;
                } else if has_content {
                  // Add spaces for empty cells after content
                  line_content.push(' ');
                }
              }
            }

            // Only add non-empty lines to reduce noise
            let trimmed = line_content.trim_end();
            if !trimmed.is_empty() {
              history_lines.push(trimmed.to_string());
            }
          }
        }
      }
    }

    AgUiTerminalContext {
      history_lines,
      cwd: cwd.to_string_lossy().to_string(),
      last_exit_code: exit_code,
    }
  }

  /// Extract context from a specific process
  pub fn extract_from_proc(
    &self,
    proc: &ProcView,
    cwd: PathBuf,
  ) -> AgUiTerminalContext {
    let mut history_lines = Vec::new();
    let exit_code = proc.exit_code().map(|code| code as i32);

    // Extract terminal buffer lines
    if let Some(vt) = &proc.vt {
      if let Ok(parser) = vt.read() {
        let screen = parser.screen();
        let size = screen.size();

        // Calculate how many rows to extract (up to max_history_lines)
        let rows_to_extract = self.max_history_lines.min(size.rows as usize);

        // Extract text row by row using the public cell() API
        for row_idx in 0..rows_to_extract {
          let mut line_content = String::new();
          let mut has_content = false;

          // Extract each cell in the row
          for col_idx in 0..size.cols {
            if let Some(cell) = screen.cell(row_idx as u16, col_idx) {
              if cell.has_contents() {
                line_content.push_str(&cell.contents());
                has_content = true;
              } else if has_content {
                // Add spaces for empty cells after content
                line_content.push(' ');
              }
            }
          }

          // Only add non-empty lines to reduce noise
          let trimmed = line_content.trim_end();
          if !trimmed.is_empty() {
            history_lines.push(trimmed.to_string());
          }
        }
      }
    }

    AgUiTerminalContext {
      history_lines,
      cwd: cwd.to_string_lossy().to_string(),
      last_exit_code: exit_code,
    }
  }

  /// Get working directory from environment
  pub fn get_cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))
  }
}

impl Default for ContextExtractor {
  fn default() -> Self {
    Self::new(500) // Default to 500 lines as per PRD
  }
}
