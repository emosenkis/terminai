use anyhow::Result;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::io::Write;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::encode_term::{KeyCodeEncodeModes, encode_key, encode_mouse_event};
use crate::key::Key;
use crate::mouse::MouseEvent;
use crate::vt100;

// Shell events
#[derive(Debug)]
pub enum ShellEvent {
  Output,
  TermReply(compact_str::CompactString),
  Exited(u32),
}

// Shell manager - simplified from mprocs' Inst
pub struct Shell {
  pub vt: Arc<RwLock<vt100::Parser<ReplySender>>>,
  pub writer: Box<dyn Write + Send>,
  pub master: Option<Box<dyn portable_pty::MasterPty + Send>>,
  pub _pid: u32,
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
    Self::spawn_internal(shell_cmd, &[], rows, cols)
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
    Self::spawn_internal(cmd, args, rows, cols)
  }

  fn spawn_internal(
    cmd: &str,
    args: &[String],
    rows: u16,
    cols: u16,
  ) -> Result<(Self, UnboundedReceiver<ShellEvent>)> {
    // Setup event channel
    let (event_tx, event_rx) = mpsc::unbounded_channel();

    // Create VT100 parser with reply sender
    let reply_sender = ReplySender {
      tx: event_tx.clone(),
    };
    let vt = vt100::Parser::new(rows, cols, 1000, reply_sender);
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
    let cwd = std::env::current_dir()?;
    log::debug!("Setting child shell CWD to: {:?}", cwd);

    // Build command
    let mut command = CommandBuilder::new(cmd);
    for arg in args {
      command.arg(arg);
    }
    command.cwd(cwd);
    command.env("TERM", "xterm-256color");
    command.env("TERMINAI", "1");

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
                  let _ = event_tx_clone.send(ShellEvent::Output);
                }
              }

              offset = chunk_end;
            }

            // Always send at least one event per read, even if no scrolling occurred
            // (in case the last chunk didn't cause scrolling but earlier ones did)
            let _ = event_tx_clone.send(ShellEvent::Output);
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
}
