// Termin.AI - Clean single-shell terminal with AI overlay
// Uses only the minimal PTY/VT100 code from mprocs, no UI chrome

use anyhow::Result;
use crossterm::{
  event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
  execute,
  terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode,
  },
};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::io::{Write, stdout};
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tui::{
  Terminal,
  backend::CrosstermBackend,
  layout::{Constraint, Direction, Layout, Rect},
  style::{Color, Style},
  widgets::{Block, Borders, Paragraph, Widget},
};

// Import only what we need from the crate
use termin::ai_proc::{AIChatProcess, AIChatUI};
use termin::encode_term::{KeyCodeEncodeModes, encode_key};
use termin::key::Key;
use termin::llm::{Provider, TerminalContext};
use termin::vt100;

// Shell events
#[derive(Debug)]
enum ShellEvent {
  Output,
  TermReply(compact_str::CompactString),
  Exited(u32),
}

// Shell manager - simplified from mprocs' Inst
struct Shell {
  vt: Arc<RwLock<vt100::Parser<ReplySender>>>,
  writer: Box<dyn Write + Send>,
  master: Option<Box<dyn portable_pty::MasterPty + Send>>,
  _pid: u32,
  event_rx: UnboundedReceiver<ShellEvent>,
}

impl Shell {
  fn spawn(shell_cmd: &str, rows: u16, cols: u16) -> Result<Self> {
    log::info!("Spawning shell: {} ({}x{})", shell_cmd, cols, rows);

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

    // Build command
    let mut cmd = CommandBuilder::new(shell_cmd);
    cmd.env("TERM", "xterm-256color");
    cmd.env("TERMINAI", "1");

    // Spawn command
    let mut child = pair.slave.spawn_command(cmd)?;
    let pid = child.process_id().unwrap_or(0);

    log::info!("Shell spawned with PID: {}", pid);

    // Get reader and writer for PTY
    let mut reader = pair.master.try_clone_reader()?;
    let writer = pair.master.take_writer()?;

    // Spawn thread to read PTY output (pattern from mprocs' inst.rs)
    let vt_clone = vt.clone();
    let event_tx_clone = event_tx.clone();
    std::thread::spawn(move || {
      let mut buf = vec![0u8; 32 * 1024];
      loop {
        match reader.read(&mut buf) {
          Ok(0) => break, // EOF
          Ok(count) => {
            // Process through VT100 parser
            if let Ok(mut vt) = vt_clone.write() {
              vt.process(&buf[..count]);
              let _ = event_tx_clone.send(ShellEvent::Output);
            }
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

    Ok(Shell {
      vt,
      writer,
      master: Some(pair.master),
      _pid: pid,
      event_rx,
    })
  }

  fn send_key(&mut self, key: Key) -> Result<()> {
    // Encode key using mprocs' encoder
    let encoded = encode_key(&key, KeyCodeEncodeModes::default())?;
    self.writer.write_all(encoded.as_bytes())?;
    self.writer.flush()?;
    Ok(())
  }

  fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
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
struct ReplySender {
  tx: UnboundedSender<ShellEvent>,
}

impl termin::vt100::TermReplySender for ReplySender {
  fn reply(&self, reply: compact_str::CompactString) {
    // Send terminal reply back to event loop to write to PTY
    let _ = self.tx.send(ShellEvent::TermReply(reply));
  }
}

// Terminal renderer widget (simplified from mprocs' UiTerm)
struct TerminalWidget<'a> {
  screen: &'a vt100::Screen<ReplySender>,
}

impl<'a> TerminalWidget<'a> {
  fn new(screen: &'a vt100::Screen<ReplySender>) -> Self {
    Self { screen }
  }
}

impl Widget for TerminalWidget<'_> {
  fn render(self, area: Rect, buf: &mut tui::buffer::Buffer) {
    // Render each cell from the VT100 screen to the tui buffer
    // Pattern borrowed from mprocs' ui_term.rs
    for row in 0..area.height {
      for col in 0..area.width {
        let pos = tui::layout::Position {
          x: area.x + col,
          y: area.y + row,
        };

        if let Some(to_cell) = buf.cell_mut(pos) {
          if let Some(cell) = self.screen.cell(row, col) {
            // Convert VT100 cell to tui cell (using mprocs' conversion)
            *to_cell = cell.to_tui();
            if !cell.has_contents() {
              to_cell.set_char(' ');
            }
          } else {
            // Out of bounds
            to_cell.set_char(' ');
          }
        }
      }
    }
  }
}

#[tokio::main]
async fn main() -> Result<()> {
  // Setup logging (enable debug for HTTP/LLM debugging)
  flexi_logger::Logger::try_with_str("info,genai=debug,reqwest=debug")?
    .log_to_file(flexi_logger::FileSpec::default())
    .start()?;

  // Detect user's shell
  let shell =
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());

  log::info!("Termin.AI starting with shell: {}", shell);

  // Create and run the app
  let mut app = App::new(shell).await?;
  app.run().await?;

  Ok(())
}

struct App {
  terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
  shell: Shell,
  ai_process: Option<AIChatProcess>,
  ai_visible: bool,
}

impl App {
  /// Extract terminal context from shell for AI
  fn extract_context(&self) -> TerminalContext {
    use std::path::PathBuf;

    let mut history_lines = Vec::new();
    let max_lines = 500; // As per PRD

    // Extract terminal buffer from VT100 screen
    if let Ok(parser) = self.shell.vt.read() {
      let screen = parser.screen();
      let size = screen.size();

      // Extract up to max_lines rows
      let rows_to_extract = max_lines.min(size.rows as usize);

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

        // Only add non-empty lines
        let trimmed = line_content.trim_end();
        if !trimmed.is_empty() {
          history_lines.push(trimmed.to_string());
        }
      }
    }

    // Get current working directory
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));

    // No exit code tracking yet (future enhancement)
    // Note: Privacy filtering will be applied by AIChatProcess.send_input_with_context
    TerminalContext::new(history_lines, cwd, None)
  }

  async fn new(shell_cmd: String) -> Result<Self> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;

    // Create ratatui terminal
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;

    // Get terminal size
    let (cols, rows) = crossterm::terminal::size()?;

    // Spawn shell
    let shell = Shell::spawn(&shell_cmd, rows, cols)?;

    // Initialize AI if API key configured
    // Try multiple providers in order of preference
    // Note: We still show the AI overlay even without a key,
    // but it will display a "not configured" message
    let ai_process = {
      let providers = [
        (Provider::Anthropic, "ANTHROPIC_API_KEY"),
        (Provider::OpenAI, "OPENAI_API_KEY"),
        (Provider::Gemini, "GOOGLE_API_KEY"),
        (Provider::Gemini, "GEMINI_API_KEY"),
        (Provider::OpenRouter, "OPENROUTER_API_KEY"),
      ];

      let mut ai = None;
      for (provider, env_key) in &providers {
        if std::env::var(env_key).is_ok() {
          log::info!("Initializing AI assistant with provider: {}", provider);

          // For OpenRouter, set the default endpoint
          let endpoint = if *provider == Provider::OpenRouter {
            Some("https://openrouter.ai/api/v1".to_string())
          } else {
            None
          };

          match AIChatProcess::new_with_endpoint(*provider, None, endpoint)
            .await
          {
            Ok(process) => {
              log::info!("AI assistant initialized successfully");
              ai = Some(process);
              break;
            }
            Err(e) => {
              log::warn!("Failed to initialize AI with {}: {:?}", provider, e);
            }
          }
        }
      }

      if ai.is_none() {
        log::info!(
          "No API keys found - AI overlay will show config instructions"
        );
      }

      ai
    };

    Ok(Self {
      terminal,
      shell,
      ai_process,
      ai_visible: false,
    })
  }

  async fn run(&mut self) -> Result<()> {
    log::info!("Termin.AI main loop starting");

    // Initial render
    self.render()?;

    loop {
      tokio::select! {
        // Handle shell events
        Some(event) = self.shell.event_rx.recv() => {
          match event {
            ShellEvent::Output => {
              // Shell produced output - don't render here to avoid performance issues
              // The VT100 parser has already been updated by the PTY reader thread
              // Rendering will happen in the periodic frame below
            }
            ShellEvent::TermReply(reply) => {
              // Write terminal query reply back to PTY so programs like glow get their responses
              if let Err(e) = self.shell.writer.write_all(reply.as_bytes()) {
                log::error!("Failed to write terminal reply: {:?}", e);
              }
              if let Err(e) = self.shell.writer.flush() {
                log::error!("Failed to flush terminal reply: {:?}", e);
              }
            }
            ShellEvent::Exited(code) => {
              log::info!("Shell exited with code: {}", code);
              break;
            }
          }
        }

        // Periodic rendering and keyboard input (60fps)
        _ = tokio::time::sleep(std::time::Duration::from_millis(16)) => {
          // Process all available keyboard events before rendering (important for paste performance)
          while event::poll(std::time::Duration::from_millis(0))? {
            match event::read()? {
              Event::Key(KeyEvent {
                code,
                modifiers,
                kind: crossterm::event::KeyEventKind::Press,
                ..
              }) => {
                // Convert to our Key type
                let key = Key::new(code, modifiers);

                // Check for hotkeys
                if matches!((code, modifiers), (KeyCode::Char(' '), KeyModifiers::CONTROL)) {
                  // Ctrl-Space: toggle AI overlay
                  self.ai_visible = !self.ai_visible;
                  log::info!("AI overlay toggled: {}", self.ai_visible);
                } else if matches!(code, KeyCode::Esc) && self.ai_visible {
                  // ESC: close AI overlay
                  self.ai_visible = false;
                } else if !self.ai_visible {
                  // Route to shell when AI overlay not visible
                  self.shell.send_key(key)?;
                } else if self.ai_process.is_some() {
                  // Route to AI overlay when visible

                  // Handle Enter key specially - needs to extract context first
                  if matches!(code, KeyCode::Enter) {
                    if let Some(ref ai_process) = self.ai_process {
                      if !ai_process.input_buffer().is_empty() {
                        log::info!("Sending message to LLM");

                        // Extract context before taking mutable borrow
                        let context = self.extract_context();

                        // Now get mutable reference and send
                        if let Some(ref mut ai_process) = self.ai_process {
                          match ai_process.send_input_with_context(context).await {
                            Ok(()) => {
                              log::info!("Message sent successfully");
                              self.render()?;
                            }
                            Err(e) => {
                              log::error!("Failed to send message: {:?}", e);
                              // Error is logged, user will see no response
                            }
                          }
                        }
                      }
                    }
                  } else if let Some(ref mut ai_process) = self.ai_process {
                    // Check if there's a pending command approval
                    if ai_process.pending_command().is_some() {
                      // Handle approval/rejection keys
                      match code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => {
                          // Approve command
                          if let Some(pending) = ai_process.approve_command() {
                            log::info!("Command approved: {}", pending.command);

                            // Execute command by injecting into shell
                            for ch in pending.command.chars() {
                              let key = Key::new(KeyCode::Char(ch), KeyModifiers::NONE);
                              if let Err(e) = self.shell.send_key(key) {
                                log::error!("Failed to send command character: {:?}", e);
                              }
                            }

                            // Send Enter to execute
                            let enter_key = Key::new(KeyCode::Enter, KeyModifiers::NONE);
                            if let Err(e) = self.shell.send_key(enter_key) {
                              log::error!("Failed to send Enter: {:?}", e);
                            }

                            log::info!("Command executed in shell");
                            self.render()?;
                          }
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') => {
                          // Reject command
                          ai_process.reject_command();
                          log::info!("Command rejected");
                        }
                        _ => {
                          // Ignore other keys when waiting for approval
                          log::debug!("Waiting for Y/N approval, ignoring key: {:?}", key);
                        }
                      }
                    } else {
                      // No pending command, handle normal input
                      match code {
                        KeyCode::Char(c)
                          if !modifiers.contains(KeyModifiers::CONTROL)
                            && !modifiers.contains(KeyModifiers::ALT) =>
                        {
                          // Regular character input (allows SHIFT for uppercase)
                          ai_process.append_input(&c.to_string());
                        }
                        KeyCode::Backspace => {
                          // Delete last character
                          ai_process.delete_char();
                        }
                        _ => {
                          // Ignore other keys when overlay is visible
                          log::debug!("Unhandled AI overlay input: {:?}", key);
                        }
                      }
                    }
                  }
                } else {
                  // AI overlay visible but no AI process (shouldn't happen, but log it)
                  log::debug!("AI overlay visible but no AI process");
                }
              }
              Event::Resize(cols, rows) => {
                self.shell.resize(rows, cols)?;
                self.render()?;
              }
              _ => {}
            }
          }

          // Render once after processing all keyboard events
          self.render()?;
        }
      }
    }

    Ok(())
  }

  fn render(&mut self) -> Result<()> {
    self.terminal.draw(|frame| {
      let area = frame.area();

      // Render shell output (full screen)
      if let Ok(vt) = self.shell.vt.read() {
        let screen = vt.screen();
        let widget = TerminalWidget::new(screen);
        frame.render_widget(widget, area);

        // Set cursor position if cursor is visible
        if !screen.hide_cursor() {
          let cursor = screen.cursor_position();
          frame.set_cursor_position((area.x + cursor.1, area.y + cursor.0));
        }
      }

      // Render AI overlay if visible
      if self.ai_visible {
        // Calculate overlay area (80% x 70%, centered)
        let overlay_area = centered_rect(80, 70, area);

        if let Some(ref ai_process) = self.ai_process {
          // Render AI chat interface
          let ai_ui = AIChatUI::new(ai_process);
          let buf = frame.buffer_mut();
          ai_ui.render(overlay_area, buf);
        } else {
          // Show "not configured" message
          let message = Paragraph::new(
            "AI Assistant not configured.\n\n\
             Set ANTHROPIC_API_KEY environment variable to enable AI features.\n\n\
             Press ESC or Ctrl-Space to close this overlay."
          )
          .block(
            Block::default()
              .borders(Borders::ALL)
              .title(" AI Assistant ")
              .style(Style::default().fg(Color::Yellow))
          )
          .style(Style::default().fg(Color::White));

          frame.render_widget(message, overlay_area);
        }
      }
    })?;
    Ok(())
  }
}

/// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
  let popup_layout = Layout::default()
    .direction(Direction::Vertical)
    .constraints(
      [
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
      ]
      .as_ref(),
    )
    .split(r);

  Layout::default()
    .direction(Direction::Horizontal)
    .constraints(
      [
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
      ]
      .as_ref(),
    )
    .split(popup_layout[1])[1]
}

impl Drop for App {
  fn drop(&mut self) {
    // Cleanup terminal
    let _ = disable_raw_mode();
    let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
  }
}
