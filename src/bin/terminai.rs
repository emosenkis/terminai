// Termin.AI - Clean single-shell terminal with AI overlay
// Uses only the minimal PTY/VT100 code from mprocs, no UI chrome

use anyhow::Result;
use clap::Parser;
use crossterm::{
  event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
  terminal::{disable_raw_mode, enable_raw_mode},
};
use std::io::{Write, stdout};
use tui::{
  Terminal, TerminalOptions, Viewport,
  backend::CrosstermBackend,
  layout::{Constraint, Direction, Layout, Rect},
  style::{Color, Style},
  widgets::{Block, Borders, Paragraph, Widget},
};

// Import only what we need from the crate
use termin::ai_proc::{AIChatProcess, AIChatUI};
use termin::key::Key;
use termin::llm::{Provider, TerminalContext};
use termin::vt100;

use termin::shell::{Shell, ShellEvent};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
  /// Command to run (if not specified, uses $SHELL)
  #[arg(last = true)]
  command: Vec<String>,
}

// Terminal renderer widget (simplified from mprocs' UiTerm)
struct TerminalWidget<'a> {
  screen: &'a vt100::Screen<termin::shell::ReplySender>,
}

impl<'a> TerminalWidget<'a> {
  fn new(screen: &'a vt100::Screen<termin::shell::ReplySender>) -> Self {
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
  // Get app cache directory
  let cache_dir = xdg::BaseDirectories::with_prefix("terminai")
    .get_cache_home()
    .map(|path| path.to_str().map(String::from))
    .flatten()
    .unwrap_or_else(|| {
      // Fallback to temporary directory if XDG not available
      std::env::temp_dir()
        .join("terminai")
        .to_string_lossy()
        .to_string()
    });

  #[cfg(debug_assertions)]
  let log_spec =
    "info,terminai=debug,genai=debug,reqwest=debug,tui_markdown=error";
  #[cfg(not(debug_assertions))]
  let log_spec = "info,genai=debug,reqwest=debug,tui_markdown=error";

  flexi_logger::Logger::try_with_env_or_str(log_spec)?
    .log_to_file(
      flexi_logger::FileSpec::default()
        .directory(&cache_dir)
        .basename("terminai")
        .suppress_timestamp(), // Don't add timestamp to filename
    )
    .append()
    .rotate(
      flexi_logger::Criterion::Size(1024 * 1024), // Rotate at 1 MB
      flexi_logger::Naming::Timestamps, // Add timestamp to rotated files
      flexi_logger::Cleanup::KeepLogFiles(5), // Keep last 5 rotated log files
    )
    .format_for_files(flexi_logger::with_thread) // Format with timestamp and thread
    .start()?;

  // Load environment variables from terminai.env (for API keys)
  // This must happen before AI initialization
  if let Err(e) = termin::env_loader::load_env_file() {
    log::error!("Failed to load terminai.env: {}", e);
    eprintln!("Error: {}", e);
    std::process::exit(1);
  }

  // Parse command line arguments
  let args = Args::parse();

  log::info!("Termin.AI starting");

  // Create and run the app
  let mut app = App::new(args.command).await?;
  app.run().await?;

  Ok(())
}

struct App<'a> {
  terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
  shell: Shell,
  ai_process: Option<AIChatProcess>,
  ai_ui: AIChatUI<'a>,
  ai_visible: bool,
  /// Track the total row count to detect when content scrolls off screen
  last_total_rows: usize,
}

impl<'a> App<'a> {
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

  async fn new(command: Vec<String>) -> Result<Self> {
    // Setup terminal
    enable_raw_mode()?;
    let stdout = stdout();

    // Get terminal size
    let (cols, rows) = crossterm::terminal::size()?;

    // Create ratatui terminal with inline viewport for native scrollback
    // This allows content to scroll into the host terminal's scrollback buffer
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::with_options(
      backend,
      TerminalOptions {
        viewport: Viewport::Inline(rows),
      },
    )?;

    // Spawn shell or command
    let shell = if command.is_empty() {
      // No command specified, use $SHELL
      let shell_cmd =
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
      log::info!("Spawning shell: {}", shell_cmd);
      Shell::spawn(&shell_cmd, rows, cols)?
    } else {
      // Command specified, spawn it directly
      let cmd = &command[0];
      let args = &command[1..];
      log::info!("Spawning command: {} {:?}", cmd, args);
      Shell::spawn_command(cmd, &args.to_vec(), rows, cols)?
    };

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
      ai_ui: AIChatUI::new(),
      ai_visible: false,
      last_total_rows: rows as usize,
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
              // Shell produced output - check if we need to render immediately
              // to prevent losing scrollback content
              // The VT100 parser has already been updated by the PTY reader thread

              // Check how many lines have scrolled since last render
              let should_render = if let Ok(vt) = self.shell.vt.read() {
                let screen = vt.screen();
                let current_total_rows = screen.total_rows();
                let screen_height = screen.size().rows as usize;
                let scrolled_since_last_render = current_total_rows.saturating_sub(self.last_total_rows);

                // If we've scrolled close to a full screen height, render immediately
                // to push content to native scrollback before it gets overwritten
                // Use 80% threshold to leave some safety margin
                let threshold = screen_height * 4 / 5;
                if scrolled_since_last_render >= threshold {
                  log::debug!(
                    "Triggering immediate render: scrolled {} lines (threshold: {})",
                    scrolled_since_last_render,
                    threshold
                  );
                  true
                } else {
                  false
                }
              } else {
                false
              };

              if should_render {
                self.render()?;
              }
              // Otherwise, rendering will happen in the periodic frame below
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
                  if matches!((code, modifiers), (KeyCode::Enter, KeyModifiers::NONE)) {
                      let message = self.ai_ui.get_input_value();
                      if !message.is_empty() {
                        log::info!("Sending message to LLM");

                        // Extract context before taking mutable borrow
                        let context = self.extract_context();

                        // Now get mutable reference and send
                        if let Some(ref mut ai_process) = self.ai_process {
                          match ai_process.send_input_with_context(&message, context).await {
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
                      if key.code() == KeyCode::Enter && key.mods() == KeyModifiers::SHIFT {
                        // TODO: Why doesn't this ever happen?
                        log::debug!("Got Shift-Enter");
                        self.ai_ui.input_event(Key::new(KeyCode::Enter, KeyModifiers::NONE));
                      } else {
                        self.ai_ui.input_event(key);
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
              Event::Mouse(mev) => {
                // TODO: Pass on mouse events to AI message input?

                // Filter out scroll events to allow native terminal scrollback
                if matches!(mev.kind, crossterm::event::MouseEventKind::ScrollUp | crossterm::event::MouseEventKind::ScrollDown) {
                  // Ignore scroll events - let the terminal handle them
                  log::debug!("Ignoring scroll event for native scrollback");
                } else {
                  // Handle other mouse events if needed in the future
                  log::debug!("Unhandled mouse event: {:?}", mev);
                }
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
    // Get current VT state and screen dimensions
    let (current_total_rows, screen_height, current_row0) =
      if let Ok(vt) = self.shell.vt.read() {
        let screen = vt.screen();
        (
          screen.total_rows(),
          screen.size().rows as usize,
          screen.row0(),
        )
      } else {
        log::error!("Failed to acquire read lock on VT");
        return Ok(());
      };

    // Calculate how much scrollback needs to be pushed to native terminal
    let total_scrolled =
      current_total_rows.saturating_sub(self.last_total_rows);

    if total_scrolled > 0 {
      log::debug!(
        "Content scrolled: {} new lines (total rows: {} -> {})",
        total_scrolled,
        self.last_total_rows,
        current_total_rows
      );

      // If we have more than one screen of scrollback to push, process it in chunks
      // to avoid losing content (we can only render screen_height lines at once)
      let mut remaining = total_scrolled;
      let mut scrollback_offset = 0;

      while remaining > 0 {
        let chunk_size = remaining.min(screen_height);

        log::debug!(
          "Rendering scrollback chunk: {} lines (remaining: {}, offset: {})",
          chunk_size,
          remaining,
          scrollback_offset
        );

        self.terminal.draw(|frame| {
          let area = frame.area();

          // Render this chunk of scrollback content
          if let Ok(vt) = self.shell.vt.read() {
            let screen = vt.screen();

            // Calculate which scrollback rows to render for this chunk
            // The oldest scrollback is at (current_row0 - total_scrolled)
            // This chunk starts at (current_row0 - total_scrolled + scrollback_offset)
            let chunk_start = current_row0
              .saturating_sub(total_scrolled)
              .saturating_add(scrollback_offset);

            let mut line_idx = 0;
            for row in screen.all_rows().skip(chunk_start).take(chunk_size) {
              if line_idx >= area.height as usize {
                break;
              }

              // Render this row into the buffer
              for col in 0..area.width.min(row.cols()) {
                if let Some(cell) = row.get(col) {
                  let buf = frame.buffer_mut();
                  if let Some(buf_cell) =
                    buf.cell_mut((area.x + col, area.y + line_idx as u16))
                  {
                    *buf_cell = cell.to_tui();
                    if !cell.has_contents() {
                      buf_cell.set_char(' ');
                    }
                  }
                }
              }
              line_idx += 1;
            }
          } else {
            log::error!(
              "Failed to acquire read lock on VT for scrollback chunk"
            );
          }

          // Scroll this chunk into native scrollback
          frame.set_scroll_up(chunk_size as u16);
        })?;

        // Update tracking
        scrollback_offset += chunk_size;
        remaining -= chunk_size;
        self.last_total_rows += chunk_size;
      }
    }

    // Final render: display current terminal state and UI widgets
    self.terminal.draw(|frame| {
      let area = frame.area();

      // Render current shell output (full screen)
      if let Ok(vt) = self.shell.vt.read() {
        let screen = vt.screen();
        let widget = TerminalWidget::new(screen);
        frame.render_widget(widget, area);

        // Set cursor position if cursor is visible
        if !self.ai_visible && !screen.hide_cursor() {
          let cursor = screen.cursor_position();
          frame.set_cursor_position((area.x + cursor.1, area.y + cursor.0));
        }
      } else {
        log::error!("Failed to acquire read lock on VT for shell");
      }

      // Render AI overlay if visible
      if self.ai_visible {
        // Calculate overlay area (80% x 70%, centered)
        let overlay_area = centered_rect(80, 70, area);

        if let Some(ref ai_process) = self.ai_process {
          // Render AI chat interface
          let buf = frame.buffer_mut();
          self.ai_ui.render(ai_process, overlay_area, buf);
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

impl<'a> Drop for App<'a> {
  fn drop(&mut self) {
    // Cleanup terminal
    let _ = disable_raw_mode();
  }
}
