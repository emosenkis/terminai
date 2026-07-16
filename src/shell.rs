use anyhow::Result;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{
  Arc, RwLock,
  atomic::{AtomicBool, Ordering},
};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::encode_term::{KeyCodeEncodeModes, encode_key, encode_mouse_event};
use crate::key::Key;
use crate::mouse::MouseEvent;
use crate::vt100;

// Shell events
#[derive(Debug)]
pub enum ShellEvent {
  Output(OutputWakeup),
  TermReply(compact_str::CompactString),
  HostEscape(compact_str::CompactString),
  Exited(u32),
}

#[derive(Clone, Debug)]
pub struct OutputWakeup {
  pending: Arc<AtomicBool>,
}

impl OutputWakeup {
  pub fn new() -> Self {
    Self {
      pending: Arc::new(AtomicBool::new(false)),
    }
  }

  fn mark_pending(&self) -> bool {
    !self.pending.swap(true, Ordering::AcqRel)
  }

  pub fn clear(&self) {
    self.pending.store(false, Ordering::Release);
  }
}

fn send_output_event(tx: &UnboundedSender<ShellEvent>, wakeup: &OutputWakeup) {
  if wakeup.mark_pending() {
    let _ = tx.send(ShellEvent::Output(wakeup.clone()));
  }
}

// Shell manager - simplified from mprocs' Inst
pub struct Shell {
  pub vt: Arc<RwLock<vt100::Parser<ReplySender>>>,
  pub writer: Box<dyn Write + Send>,
  pub master: Option<Box<dyn portable_pty::MasterPty + Send>>,
  pub _pid: u32,
}

#[derive(Debug, Clone, Default)]
pub struct ShellSpawnOptions {
  pub cwd: Option<PathBuf>,
  pub env: HashMap<String, String>,
  pub scrollback_len: usize,
}

impl Shell {
  /// Spawn a shell command (runs through /bin/sh -c)
  /// Returns (Shell, event_receiver) tuple
  pub fn spawn(
    shell_cmd: &str,
    rows: u16,
    cols: u16,
  ) -> Result<(Self, UnboundedReceiver<ShellEvent>)> {
    log::info!("Spawning shell: {} ({}x{})", shell_cmd, cols, rows);
    Self::spawn_internal(
      shell_cmd,
      &[],
      rows,
      cols,
      ShellSpawnOptions::default(),
    )
  }

  /// Spawn a command with explicit arguments (no shell interpretation)
  /// Returns (Shell, event_receiver) tuple
  pub fn spawn_command(
    cmd: &str,
    args: &[String],
    rows: u16,
    cols: u16,
  ) -> Result<(Self, UnboundedReceiver<ShellEvent>)> {
    log::info!("Spawning command: {} {:?} ({}x{})", cmd, args, cols, rows);
    Self::spawn_internal(cmd, args, rows, cols, ShellSpawnOptions::default())
  }

  /// Spawn a command with explicit arguments and process options.
  /// Returns (Shell, event_receiver) tuple.
  pub fn spawn_command_with_options(
    cmd: &str,
    args: &[String],
    rows: u16,
    cols: u16,
    options: ShellSpawnOptions,
  ) -> Result<(Self, UnboundedReceiver<ShellEvent>)> {
    log::info!(
      "Spawning command with options: {} {:?} ({}x{})",
      cmd,
      args,
      cols,
      rows
    );
    Self::spawn_internal(cmd, args, rows, cols, options)
  }

  fn spawn_internal(
    cmd: &str,
    args: &[String],
    rows: u16,
    cols: u16,
    options: ShellSpawnOptions,
  ) -> Result<(Self, UnboundedReceiver<ShellEvent>)> {
    // Setup event channel
    let (event_tx, event_rx) = mpsc::unbounded_channel();

    // Create VT100 parser with reply sender
    let reply_sender = ReplySender {
      tx: event_tx.clone(),
    };
    let scrollback_len = if options.scrollback_len == 0 {
      1000
    } else {
      options.scrollback_len
    };
    let vt = vt100::Parser::new(rows, cols, scrollback_len, reply_sender);
    let vt = Arc::new(RwLock::new(vt));

    // Create PTY (using portable-pty like mprocs)
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
      rows,
      cols,
      pixel_width: 0,
      pixel_height: 0,
    })?;

    // Get current working directory to pass to child shell
    let cwd = match options.cwd {
      Some(cwd) => cwd,
      None => std::env::current_dir()?,
    };
    log::debug!("Setting child shell CWD to: {:?}", cwd);

    // Build command
    let mut command = CommandBuilder::new(cmd);
    for arg in args {
      command.arg(arg);
    }
    command.cwd(cwd);
    command.env("TERM", "xterm-256color");
    command.env("TERMINAI", "1");
    for (key, value) in options.env {
      command.env(key, value);
    }

    // Spawn command
    let mut child = pair.slave.spawn_command(command)?;
    let pid = child.process_id().unwrap_or(0);

    log::info!("Command spawned with PID: {}", pid);

    // Get reader and writer for PTY
    let mut reader = pair.master.try_clone_reader()?;
    let writer = pair.master.take_writer()?;

    // Spawn thread to read PTY output (pattern from mprocs' inst.rs)
    let vt_clone = vt.clone();
    let event_tx_clone = event_tx.clone();
    let output_wakeup = OutputWakeup::new();
    std::thread::spawn(move || {
      let mut buf = vec![0u8; 32 * 1024];

      // Calculate chunk size for incremental processing
      // Use ~1/2 screen of characters as chunk size to trigger events frequently enough
      // to catch scrolling before too much accumulates
      let chunk_size = (rows as usize * cols as usize / 2).max(4096);

      loop {
        match reader.read(&mut buf) {
          Ok(0) => break, // EOF
          Ok(count) => {
            // Process through VT100 parser in chunks, sending events between chunks
            // to allow the main loop to catch scrollback before too much accumulates
            let data = &buf[..count];
            let mut offset = 0;

            while offset < data.len() {
              let chunk_end = (offset + chunk_size).min(data.len());
              let chunk = &data[offset..chunk_end];

              if let Ok(mut vt) = vt_clone.write() {
                let rows_before = vt.screen().total_rows();
                vt.process(chunk);
                let rows_after = vt.screen().total_rows();

                // Send event if this chunk caused scrolling
                if rows_after > rows_before {
                  send_output_event(&event_tx_clone, &output_wakeup);
                }
              }

              offset = chunk_end;
            }

            // Always send at least one event per read, even if no scrolling occurred
            // (in case the last chunk didn't cause scrolling but earlier ones did)
            send_output_event(&event_tx_clone, &output_wakeup);
          }
          Err(e) => {
            log::error!("PTY read error: {}", e);
            break;
          }
        }
      }
    });

    // Spawn thread to wait for child exit
    std::thread::spawn(move || {
      let exit_code = match child.wait() {
        Ok(status) => status.exit_code(),
        Err(_) => 1,
      };
      let _ = event_tx.send(ShellEvent::Exited(exit_code));
    });

    Ok((
      Shell {
        vt,
        writer,
        master: Some(pair.master),
        _pid: pid,
      },
      event_rx,
    ))
  }

  pub fn send_key(&mut self, key: Key) -> Result<()> {
    // Encode key using mprocs' encoder
    let encoded = encode_key(&key, KeyCodeEncodeModes::default())?;
    self.writer.write_all(encoded.as_bytes())?;
    self.writer.flush()?;
    Ok(())
  }

  /// Send pasted text to the shell
  ///
  /// If the guest shell has enabled bracketed paste mode (ESC[?2004h),
  /// the pasted text will be wrapped with the bracketed paste sequences
  /// (ESC[200~ ... ESC[201~). Otherwise, the text is sent directly.
  pub fn send_paste(&mut self, text: &str) -> Result<()> {
    let use_bracketed_paste = self
      .vt
      .read()
      .map(|vt| vt.screen().bracketed_paste())
      .unwrap_or(false);

    if use_bracketed_paste {
      // Wrap with bracketed paste sequences
      self.writer.write_all(b"\x1b[200~")?;
      self.writer.write_all(text.as_bytes())?;
      self.writer.write_all(b"\x1b[201~")?;
    } else {
      // Send text directly
      self.writer.write_all(text.as_bytes())?;
    }
    self.writer.flush()?;
    Ok(())
  }

  /// Send a command string to the shell
  ///
  /// Decodes escape sequences in the command string before sending:
  /// - \r -> carriage return (0x0D)
  /// - \n -> newline (0x0A)
  /// - \t -> tab (0x09)
  /// - \b -> backspace (0x08)
  /// - \u00XX -> unicode character (e.g., \u001b for ESC, \u0003 for Ctrl-C)
  ///
  /// If the command doesn't end with \r or \n, no Enter key is appended.
  pub fn send_command(&mut self, command: &str) -> Result<()> {
    // Decode escape sequences
    let decoded = Self::decode_escape_sequences(command);

    // Write the decoded bytes
    self.writer.write_all(&decoded)?;
    self.writer.flush()?;
    Ok(())
  }

  /// Decode common escape sequences in a string to their byte equivalents
  fn decode_escape_sequences(s: &str) -> Vec<u8> {
    let mut result = Vec::new();
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
      if ch == '\\' {
        match chars.peek() {
          Some('r') => {
            chars.next();
            result.push(0x0D); // \r -> CR
          }
          Some('n') => {
            chars.next();
            result.push(0x0A); // \n -> LF
          }
          Some('t') => {
            chars.next();
            result.push(0x09); // \t -> TAB
          }
          Some('b') => {
            chars.next();
            result.push(0x08); // \b -> BS
          }
          Some('u') => {
            chars.next();
            // Try to parse \uXXXX format
            let hex_chars: String = chars.by_ref().take(4).collect();
            if hex_chars.len() == 4 {
              if let Ok(code) = u16::from_str_radix(&hex_chars, 16) {
                if code <= 0xFF {
                  // Only support ASCII/Latin-1 range for terminal control
                  result.push(code as u8);
                } else {
                  // Invalid for terminal control - encode as UTF-8
                  if let Some(unicode_char) = char::from_u32(code as u32) {
                    let mut buf = [0u8; 4];
                    let encoded = unicode_char.encode_utf8(&mut buf);
                    result.extend_from_slice(encoded.as_bytes());
                  }
                }
              } else {
                // Failed to parse - keep literal
                result.push(b'\\');
                result.push(b'u');
                result.extend_from_slice(hex_chars.as_bytes());
              }
            } else {
              // Not enough hex digits - keep literal
              result.push(b'\\');
              result.push(b'u');
              result.extend_from_slice(hex_chars.as_bytes());
            }
          }
          Some('\\') => {
            chars.next();
            result.push(b'\\'); // \\ -> \
          }
          _ => {
            // Unknown escape - keep literal backslash
            result.push(b'\\');
          }
        }
      } else {
        // Regular character - encode as UTF-8
        let mut buf = [0u8; 4];
        let encoded = ch.encode_utf8(&mut buf);
        result.extend_from_slice(encoded.as_bytes());
      }
    }

    result
  }

  pub fn send_mouse(&mut self, event: MouseEvent) -> Result<()> {
    // Check if the terminal has enabled mouse reporting
    if let Ok(vt) = self.vt.read() {
      let mouse_mode = vt.screen().mouse_protocol_mode();
      match mouse_mode {
        vt100::MouseProtocolMode::None => {
          // Mouse reporting not enabled, don't send anything
        }
        vt100::MouseProtocolMode::Press
        | vt100::MouseProtocolMode::PressRelease
        | vt100::MouseProtocolMode::ButtonMotion
        | vt100::MouseProtocolMode::AnyMotion => {
          // Encode and send mouse event to PTY
          let encoded = encode_mouse_event(event);
          self.writer.write_all(encoded.as_bytes())?;
          self.writer.flush()?;
        }
      }
    }
    Ok(())
  }

  pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
    // Resize PTY (pattern from mprocs' inst.rs)
    if let Some(master) = &self.master {
      master.resize(PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
      })?;
    }

    // Resize VT100 parser
    if let Ok(mut vt) = self.vt.write() {
      vt.set_size(rows, cols);
    }

    log::info!("Shell resized to {}x{}", cols, rows);
    Ok(())
  }
}

// Reply sender for VT100 terminal queries
#[derive(Clone)]
pub struct ReplySender {
  tx: UnboundedSender<ShellEvent>,
}

impl crate::vt100::TermReplySender for ReplySender {
  fn reply(&self, reply: compact_str::CompactString) {
    // Send terminal reply back to event loop to write to PTY
    let _ = self.tx.send(ShellEvent::TermReply(reply));
  }

  fn host_escape(&self, escape: compact_str::CompactString) {
    let _ = self.tx.send(ShellEvent::HostEscape(escape));
  }
}

#[cfg(all(test, windows))]
mod windows_smoke_tests {
  use super::*;
  use std::time::{Duration, Instant};

  fn screen_text(shell: &Shell, cols: u16) -> String {
    shell
      .vt
      .read()
      .unwrap()
      .screen()
      .all_rows()
      .flat_map(|row| (0..cols).filter_map(move |col| row.get(col)))
      .map(|cell| cell.contents())
      .collect()
  }

  fn wait_for_output_and_exit(
    shell: &Shell,
    events: &mut UnboundedReceiver<ShellEvent>,
    expected_output: &str,
    cols: u16,
  ) -> (String, Option<u32>) {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut exit_code = None;
    while Instant::now() < deadline {
      while let Ok(event) = events.try_recv() {
        match event {
          ShellEvent::Output(wakeup) => wakeup.clear(),
          ShellEvent::Exited(code) => exit_code = Some(code),
          ShellEvent::TermReply(_) | ShellEvent::HostEscape(_) => {}
        }
      }
      let text = screen_text(shell, cols);
      if text.contains(expected_output) && exit_code.is_some() {
        return (text, exit_code);
      }
      std::thread::sleep(Duration::from_millis(10));
    }
    (screen_text(shell, cols), exit_code)
  }

  #[test]
  fn conpty_cmd_echo_resize_and_exit() {
    let (mut shell, mut events) = Shell::spawn_command(
      "cmd.exe",
      &["/C".into(), "echo terminai-conpty".into()],
      24,
      80,
    )
    .expect("cmd.exe should spawn through ConPTY");
    shell.resize(30, 100).expect("ConPTY should resize");
    let (text, exit_code) =
      wait_for_output_and_exit(&shell, &mut events, "terminai-conpty", 100);
    assert_eq!(
      exit_code,
      Some(0),
      "cmd.exe did not exit cleanly; captured output: {text:?}"
    );
    assert!(
      text.contains("terminai-conpty"),
      "captured output: {text:?}"
    );
  }

  #[test]
  fn conpty_powershell_output_when_available() {
    if which::which("powershell.exe").is_err() {
      return;
    }
    let (shell, mut events) = Shell::spawn_command(
      "powershell.exe",
      &[
        "-NoProfile".into(),
        "-Command".into(),
        "Write-Output terminai-powershell".into(),
      ],
      24,
      80,
    )
    .expect("powershell.exe should spawn through ConPTY");
    let (text, exit_code) =
      wait_for_output_and_exit(&shell, &mut events, "terminai-powershell", 80);
    assert_eq!(
      exit_code,
      Some(0),
      "powershell.exe did not exit cleanly; captured output: {text:?}"
    );
    assert!(
      text.contains("terminai-powershell"),
      "captured output: {text:?}"
    );
  }
}
