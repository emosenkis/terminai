//! Application initialization functions for Terminai binary
//!
//! This module provides setup functions extracted from main() to improve
//! testability and separation of concerns:
//!
//! - [`setup_logging`]: Configure file-based logging with rotation
//! - [`create_terminal`]: Create the rat-salsa terminal
//! - [`get_cache_dir`]: Get the XDG cache directory for terminai
//! - [`get_log_path`]: Get the full path to the log file

use anyhow::Result;
use crossterm::cursor::SetCursorStyle;
use crossterm::event::KeyboardEnhancementFlags;
#[cfg(unix)]
use crossterm::terminal::{
  disable_raw_mode, enable_raw_mode, is_raw_mode_enabled,
};
use flexi_logger::{Cleanup, Criterion, FileSpec, Naming};
use rat_salsa::terminal::{CrosstermTerminal, SalsaOptions};
use std::io::{IsTerminal, stdout};
#[cfg(unix)]
use std::{
  fs::OpenOptions,
  io::{Read, Write},
  os::fd::AsRawFd,
  time::{Duration, Instant},
};
use tui::{
  Terminal, TerminalOptions, Viewport,
  backend::{Backend, CrosstermBackend},
};

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

/// Windows Terminai requires a VT-capable console. Windows Terminal provides
/// this; legacy Console Host and redirected output are deliberately rejected.
#[cfg(windows)]
pub fn require_windows_terminal() -> Result<()> {
  use winapi::um::consoleapi::{GetConsoleMode, SetConsoleMode};
  use winapi::um::processenv::GetStdHandle;
  use winapi::um::winbase::STD_OUTPUT_HANDLE;
  const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
  if !std::io::stdout().is_terminal() {
    anyhow::bail!("Windows Terminal is required: stdout is not a console")
  }
  unsafe {
    let handle = GetStdHandle(STD_OUTPUT_HANDLE);
    let mut mode = 0;
    if handle.is_null()
      || GetConsoleMode(handle, &mut mode) == 0
      || SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING) == 0
    {
      anyhow::bail!(
        "Windows Terminal is required: unable to enable VT output processing"
      )
    }
  }
  Ok(())
}

#[cfg(not(windows))]
pub fn require_windows_terminal() -> Result<()> {
  Ok(())
}

pub(crate) fn terminal_options(synchronized_output: bool) -> SalsaOptions {
  SalsaOptions {
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
    synchronized_output,
    ratatui_options: TerminalOptions {
      viewport: Viewport::Fullscreen,
    },
    ..Default::default()
  }
}

pub fn should_enable_terminal_sync(
  configured: bool,
  host_supported: bool,
) -> bool {
  configured && host_supported
}

/// Query DEC private mode 2026 (DECRQM). Unknown modes are reported as 0 or 4.
#[cfg(unix)]
pub fn supports_synchronized_output() -> bool {
  let was_raw = is_raw_mode_enabled().unwrap_or(false);
  if !was_raw && enable_raw_mode().is_err() {
    return false;
  }

  let result = OpenOptions::new()
    .read(true)
    .write(true)
    .open("/dev/tty")
    .and_then(|mut tty| {
      query_synchronized_output_on(&mut tty, Duration::from_millis(250))
    })
    .unwrap_or(false);

  if !was_raw {
    let _ = disable_raw_mode();
  }
  result
}

#[cfg(not(unix))]
pub fn supports_synchronized_output() -> bool {
  false
}

#[cfg(unix)]
fn query_synchronized_output_on<T>(
  tty: &mut T,
  timeout: Duration,
) -> std::io::Result<bool>
where
  T: Read + Write + AsRawFd,
{
  const QUERY: &[u8] = b"\x1b[?2026$p\x1b[c";

  tty.write_all(QUERY)?;
  tty.flush()?;

  let deadline = Instant::now() + timeout;
  let mut response = Vec::new();
  let mut chunk = [0; 128];
  loop {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
      return Ok(false);
    }
    let mut descriptor = libc::pollfd {
      fd: tty.as_raw_fd(),
      events: libc::POLLIN,
      revents: 0,
    };
    let ready = unsafe {
      libc::poll(
        &mut descriptor,
        1,
        remaining.as_millis().min(i32::MAX as u128) as i32,
      )
    };
    if ready < 0 {
      return Err(std::io::Error::last_os_error());
    }
    if ready == 0 {
      return Ok(false);
    }

    let read = tty.read(&mut chunk)?;
    if read == 0 {
      return Ok(false);
    }
    response.extend_from_slice(&chunk[..read]);

    if let Some(supported) = synchronized_output_report(&response) {
      return Ok(supported);
    }
    if has_primary_device_attributes_report(&response) {
      return Ok(false);
    }
  }
}

fn synchronized_output_report(response: &[u8]) -> Option<bool> {
  csi_sequences(response).find_map(|sequence| {
    let parameters = sequence.strip_suffix(b"$y")?;
    let value = parameters.strip_prefix(b"?2026;")?;
    match value {
      b"1" | b"2" => Some(true),
      b"0" | b"3" | b"4" => Some(false),
      _ => None,
    }
  })
}

fn has_primary_device_attributes_report(response: &[u8]) -> bool {
  csi_sequences(response).any(|sequence| sequence.ends_with(b"c"))
}

fn csi_sequences(response: &[u8]) -> impl Iterator<Item = &[u8]> {
  response
    .windows(2)
    .enumerate()
    .filter(|(_, prefix)| *prefix == b"\x1b[")
    .filter_map(|(start, _)| {
      let sequence = &response[start + 2..];
      let end = sequence
        .iter()
        .position(|byte| (0x40..=0x7e).contains(byte))?;
      Some(&sequence[..=end])
    })
}

fn create_ratatui_terminal_with_options<B: Backend>(
  backend: B,
  options: &SalsaOptions,
) -> Result<Terminal<B>> {
  Ok(Terminal::with_options(
    backend,
    options.ratatui_options.clone(),
  )?)
}

pub(crate) fn create_ratatui_terminal<B: Backend>(
  backend: B,
) -> Result<Terminal<B>> {
  create_ratatui_terminal_with_options(backend, &terminal_options(true))
}

/// Create the terminal on the main screen with native scrollback support.
pub fn create_terminal(synchronized_output: bool) -> Result<CrosstermTerminal> {
  let options = terminal_options(synchronized_output);
  let terminal = create_ratatui_terminal_with_options(
    CrosstermBackend::new(stdout()),
    &options,
  )?;
  Ok(CrosstermTerminal::from_ratatui_terminal(terminal, options))
}

/// Get the cache directory for terminai
pub fn get_cache_dir() -> String {
  crate::paths::cache_dir()
    .ok()
    .and_then(|path| path.to_str().map(String::from))
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

  #[test]
  fn terminal_sync_option_is_forwarded() {
    assert!(terminal_options(true).synchronized_output);
    assert!(!terminal_options(false).synchronized_output);
  }

  #[test]
  fn terminal_sync_requires_config_and_host_support() {
    assert!(should_enable_terminal_sync(true, true));
    assert!(!should_enable_terminal_sync(true, false));
    assert!(!should_enable_terminal_sync(false, true));
    assert!(!should_enable_terminal_sync(false, false));
  }

  #[test]
  fn synchronized_output_reports_are_parsed() {
    assert_eq!(synchronized_output_report(b"\x1b[?2026;1$y"), Some(true));
    assert_eq!(synchronized_output_report(b"\x1b[?2026;2$y"), Some(true));
    assert_eq!(synchronized_output_report(b"\x1b[?2026;0$y"), Some(false));
    assert_eq!(synchronized_output_report(b"\x1b[?2026;4$y"), Some(false));
    assert_eq!(synchronized_output_report(b"\x1b[?2026;2$"), None);
  }

  #[cfg(unix)]
  #[test]
  fn synchronized_output_query_uses_the_host_report() {
    use std::os::unix::net::UnixStream;
    use std::thread;

    for (response, expected) in [
      (b"\x1b[?2026;2$y".as_slice(), true),
      (b"\x1b[?1;2c".as_slice(), false),
    ] {
      let (mut client, mut host) = UnixStream::pair().unwrap();
      let responder = thread::spawn(move || {
        let mut query = [0; 12];
        host.read_exact(&mut query).unwrap();
        assert_eq!(&query, b"\x1b[?2026$p\x1b[c");
        host.write_all(response).unwrap();
      });

      assert_eq!(
        query_synchronized_output_on(&mut client, Duration::from_secs(1))
          .unwrap(),
        expected
      );
      responder.join().unwrap();
    }
  }
}
