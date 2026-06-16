//! Application initialization functions for Termin.AI binary
//!
//! This module provides setup functions extracted from main() to improve
//! testability and separation of concerns:
//!
//! - [`setup_logging`]: Configure file-based logging with rotation
//! - [`create_terminal`]: Create the rat-salsa terminal with inline viewport
//! - [`get_cache_dir`]: Get the XDG cache directory for terminai
//! - [`get_log_path`]: Get the full path to the log file

use anyhow::Result;
use crossterm::cursor::SetCursorStyle;
use crossterm::event::KeyboardEnhancementFlags;
use flexi_logger::{Cleanup, Criterion, FileSpec, Naming};
use rat_salsa::terminal::{CrosstermTerminal, SalsaOptions};
use tui::{TerminalOptions, Viewport};

/// Setup logging to file with rotation
pub fn setup_logging() -> Result<()> {
  // Get app cache directory
  let cache_dir = get_cache_dir();

  #[cfg(debug_assertions)]
  let log_spec = "info,terminai=debug,tui_markdown=error";
  #[cfg(not(debug_assertions))]
  let log_spec = "info,tui_markdown=error";

  flexi_logger::Logger::try_with_env_or_str(log_spec)?
    .log_to_file(
      FileSpec::default()
        .directory(&cache_dir)
        .basename("terminai")
        .suppress_timestamp(), // Don't add timestamp to filename
    )
    .append()
    .rotate(
      Criterion::Size(1024 * 1024), // Rotate at 1 MB
      Naming::Timestamps,           // Add timestamp to rotated files
      Cleanup::KeepLogFiles(5),     // Keep last 5 rotated log files
    )
    .format_for_files(flexi_logger::with_thread) // Format with timestamp and thread
    .start()?;

  Ok(())
}

/// Create the terminal with inline mode (native scrollback support)
pub fn create_terminal() -> Result<CrosstermTerminal> {
  let (_, rows) = crossterm::terminal::size()?;

  let options = SalsaOptions {
    alternate_screen: false,
    mouse_capture: false, // Don't capture mouse - allow native scrolling
    bracketed_paste: true,
    cursor_blinking: true,
    cursor: SetCursorStyle::DefaultUserShape,
    keyboard_enhancements: KeyboardEnhancementFlags::REPORT_EVENT_TYPES
      | KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
      | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
      | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES,
    shutdown_clear: false,
    ratatui_options: TerminalOptions {
      viewport: Viewport::Inline(rows),
    },
    ..Default::default()
  };

  Ok(CrosstermTerminal::with_options(options)?)
}

/// Get the cache directory for terminai
pub fn get_cache_dir() -> String {
  xdg::BaseDirectories::with_prefix("terminai")
    .get_cache_home()
    .map(|path| path.to_str().map(String::from))
    .flatten()
    .unwrap_or_else(|| {
      // Fallback to temporary directory if XDG not available
      std::env::temp_dir()
        .join("terminai")
        .to_string_lossy()
        .to_string()
    })
}

/// Get the log file path for error messages
pub fn get_log_path() -> String {
  format!("{}/terminai.log", get_cache_dir())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_get_cache_dir() {
    let dir = get_cache_dir();
    assert!(dir.contains("terminai") || dir.contains("tmp"));
  }

  #[test]
  fn test_get_log_path() {
    let path = get_log_path();
    assert!(path.contains("terminai"));
    assert!(path.ends_with(".log"));
  }
}
