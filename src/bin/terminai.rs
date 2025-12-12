// Termin.AI - Clean single-shell terminal with AI overlay
// Uses only the minimal PTY/VT100 code from mprocs, no UI chrome

use anyhow::{Error, Result};
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::io::{Write, stdout};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tui::{
  Terminal, TerminalOptions, Viewport,
  backend::CrosstermBackend,
  layout::{Constraint, Direction, Layout, Rect},
  style::{Color, Style},
  widgets::{Block, Borders, Clear, Paragraph, Widget},
};

// rat-salsa imports
use rat_focus::{FocusBuilder, FocusFlag, match_focus};
use rat_salsa::{
  Control, RunConfig, SalsaAppContext, SalsaContext,
  poll::{PollEvents, PollRendered},
  run_tui,
};
use rat_theme4::{create_salsa_theme, theme::SalsaTheme};

// Import only what we need from the crate
use termin::ai_proc::{AIChatProcess, AIChatUI};
use termin::key::Key;
use termin::llm::{Provider, TerminalContext};
use termin::terminai_config::TerminAIConfig;
use termin::vt100;

use termin::shell::{Shell, ShellEvent};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
  /// Command to run (if not specified, uses $SHELL)
  #[arg(last = true)]
  command: Vec<String>,
}

/// Global state for rat-salsa (implements SalsaContext)
pub struct Global {
  ctx: SalsaAppContext<AppEvent, Error>,
  theme: SalsaTheme,
}

impl SalsaContext<AppEvent, Error> for Global {
  fn set_salsa_ctx(&mut self, app_ctx: SalsaAppContext<AppEvent, Error>) {
    self.ctx = app_ctx;
  }

  fn salsa_ctx(&self) -> &SalsaAppContext<AppEvent, Error> {
    &self.ctx
  }
}

impl Global {
  pub fn new(theme: SalsaTheme) -> Self {
    Self {
      ctx: Default::default(),
      theme,
    }
  }
}

/// Application events
#[derive(Debug)]
pub enum AppEvent {
  /// Post-render event (for focus rebuild)
  Rendered,

  /// Shell events
  ShellOutput,
  ShellTermReply(String),
  ShellExited(i32),
}

impl From<rat_salsa::event::RenderedEvent> for AppEvent {
  fn from(_: rat_salsa::event::RenderedEvent) -> Self {
    Self::Rendered
  }
}

/// Custom event source for shell events
pub struct PollShell {
  receiver: Arc<Mutex<Option<mpsc::UnboundedReceiver<ShellEvent>>>>,
  cached_event: Arc<Mutex<Option<ShellEvent>>>,
}

impl PollShell {
  pub fn new(receiver: mpsc::UnboundedReceiver<ShellEvent>) -> Self {
    Self {
      receiver: Arc::new(Mutex::new(Some(receiver))),
      cached_event: Arc::new(Mutex::new(None)),
    }
  }
}

impl PollEvents<AppEvent, Error> for PollShell {
  fn as_any(&self) -> &dyn std::any::Any {
    self
  }

  fn poll(&mut self) -> Result<bool, Error> {
    // Check if we have a cached event
    if self.cached_event.lock().unwrap().is_some() {
      return Ok(true);
    }

    // Try to receive a new event and cache it
    if let Some(ref mut rx) = *self.receiver.lock().unwrap() {
      match rx.try_recv() {
        Ok(event) => {
          *self.cached_event.lock().unwrap() = Some(event);
          Ok(true)
        }
        Err(mpsc::error::TryRecvError::Empty) => Ok(false),
        Err(mpsc::error::TryRecvError::Disconnected) => {
          // Shell died - cache a synthetic exit event
          *self.cached_event.lock().unwrap() = Some(ShellEvent::Exited(1));
          Ok(true)
        }
      }
    } else {
      Ok(false)
    }
  }

  fn read(&mut self) -> Result<Control<AppEvent>, Error> {
    // Read and consume the cached event
    if let Some(event) = self.cached_event.lock().unwrap().take() {
      match event {
        ShellEvent::Output => Ok(Control::Event(AppEvent::ShellOutput)),
        ShellEvent::TermReply(reply) => {
          Ok(Control::Event(AppEvent::ShellTermReply(reply.to_string())))
        }
        ShellEvent::Exited(code) => {
          Ok(Control::Event(AppEvent::ShellExited(code as i32)))
        }
      }
    } else {
      Ok(Control::Continue)
    }
  }
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

/// Helper to initialize shell and AI process asynchronously
async fn initialize_app_components(
  command: Vec<String>,
) -> Result<(Shell, Option<AIChatProcess>)> {
  // Get terminal size
  let (cols, rows) = crossterm::terminal::size()?;

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

  // Initialize AI (same logic as before)
  let ai_process = initialize_ai().await;

  Ok((shell, ai_process))
}

/// Initialize AI process (extracted from App::new)
async fn initialize_ai() -> Option<AIChatProcess> {
  match TerminAIConfig::load() {
    Ok(config) => {
      log::info!("Configuration loaded successfully");

      match config.get_default_provider_and_model() {
        Ok((provider_config, model_config)) => {
          log::info!(
            "Using configured provider: {} with model: {}",
            provider_config.name,
            model_config.name
          );

          let api_key_env = provider_config.effective_api_key_env();
          if let Some(ref env_key) = api_key_env {
            if std::env::var(env_key).is_ok() {
              if let Ok(provider) =
                std::str::FromStr::from_str(&provider_config.name)
              {
                let endpoint = if provider == Provider::OpenRouter {
                  Some("https://openrouter.ai/api/v1".to_string())
                } else {
                  None
                };

                match AIChatProcess::new_with_endpoint(
                  provider,
                  Some(model_config.model.clone()),
                  endpoint,
                )
                .await
                {
                  Ok(process) => {
                    log::info!("AI assistant initialized successfully");
                    return Some(process);
                  }
                  Err(e) => {
                    log::error!(
                      "Failed to initialize AI with configured provider: {:?}",
                      e
                    );
                  }
                }
              } else {
                log::error!(
                  "Unknown provider in config: {}",
                  provider_config.name
                );
              }
            } else {
              log::warn!("API key environment variable {} not set", env_key);
            }
          } else {
            log::warn!("No API key environment variable configured");
          }
        }
        Err(e) => {
          log::error!(
            "Failed to get default provider/model from config: {:?}",
            e
          );
        }
      }
    }
    Err(e) => {
      log::info!("No config file found or failed to load: {:?}", e);
      log::info!("Falling back to auto-detection of API keys");

      // Fallback: Try multiple providers
      let providers = [
        (Provider::Anthropic, "ANTHROPIC_API_KEY"),
        (Provider::OpenAI, "OPENAI_API_KEY"),
        (Provider::Gemini, "GOOGLE_API_KEY"),
        (Provider::Gemini, "GEMINI_API_KEY"),
        (Provider::OpenRouter, "OPENROUTER_API_KEY"),
      ];

      for (provider, env_key) in &providers {
        if std::env::var(env_key).is_ok() {
          log::info!("Initializing AI assistant with provider: {}", provider);

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
              return Some(process);
            }
            Err(e) => {
              log::warn!("Failed to initialize AI with {}: {:?}", provider, e);
            }
          }
        }
      }

      log::info!(
        "No API keys found - AI overlay will show config instructions"
      );
    }
  }

  None
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

  // Initialize shell and AI asynchronously
  let (shell, ai_process) = initialize_app_components(args.command).await?;

  // Get terminal size for initial state
  let (_, rows) = crossterm::terminal::size()?;

  // Create theme
  let theme = create_salsa_theme("Monochrome Dark");
  let mut global = Global::new(theme);

  // Create application state
  let mut state = AppState {
    shell,
    ai_process,
    ai_ui: AIChatUI::new(),
    ai_visible: false,
    last_total_rows: rows as usize,
    focus_conversation: FocusFlag::default(),
    focus_input: FocusFlag::default(),
  };

  // Run rat-salsa event loop
  // NOTE: For Phase 1, we poll crossterm events manually to avoid version conflicts
  // NOTE: Shell events are also polled inline in the event() function
  // TODO: Phase 2+: Use PollCrossterm and PollShell properly
  run_tui(
    init,
    render,
    event,
    error,
    &mut global,
    &mut state,
    RunConfig::default()?.poll(PollRendered),
  )?;

  Ok(())
}

/// Application state (previously App)
struct AppState<'a> {
  shell: Shell,
  ai_process: Option<AIChatProcess>,
  ai_ui: AIChatUI<'a>,
  ai_visible: bool,
  /// Track the total row count to detect when content scrolls off screen
  last_total_rows: usize,
  /// Focus flags for AI modal components (Phase 5)
  focus_conversation: FocusFlag,
  focus_input: FocusFlag,
}

impl<'a> AppState<'a> {
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

  // OLD METHODS REMOVED - now using rat-salsa init/render/event functions instead
  // See init(), render(), event(), error() functions below
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

/// rat-salsa init function - initialize focus and state
pub fn init(state: &mut AppState, _ctx: &mut Global) -> Result<(), Error> {
  // Initialize focus (Phase 5)
  // Focus is only active when AI modal is visible
  if state.ai_visible {
    let mut builder = FocusBuilder::default();
    builder.widget(&state.focus_conversation);
    builder.widget(&state.focus_input);
    let focus = builder.build();
    // Focus on input by default when modal opens
    focus.focus(&state.focus_input);
  }
  Ok(())
}

/// rat-salsa render function - render the UI
pub fn render(
  area: Rect,
  buf: &mut tui::buffer::Buffer,
  state: &mut AppState,
  _ctx: &mut Global,
) -> Result<(), Error> {
  // Render shell terminal (always visible as background)
  if let Ok(vt) = state.shell.vt.read() {
    let screen = vt.screen();
    let widget = TerminalWidget::new(screen);
    widget.render(area, buf);
  }

  // Render AI overlay if visible (Phase 2)
  if state.ai_visible {
    // Calculate overlay area (80% x 70%, centered)
    let overlay_area = centered_rect(80, 70, area);

    if let Some(ref ai_process) = state.ai_process {
      // Render AI chat interface with focus flags (Phase 5)
      state.ai_ui.render(
        ai_process,
        overlay_area,
        buf,
        &state.focus_conversation,
        &state.focus_input,
      );
    } else {
      // Show "not configured" message
      let message = Paragraph::new(
        "AI Assistant not configured.\n\n\
         Set ANTHROPIC_API_KEY environment variable to enable AI features.\n\n\
         Press ESC or Ctrl-Space to close this overlay.",
      )
      .block(
        Block::default()
          .borders(Borders::ALL)
          .title(" AI Assistant ")
          .style(Style::default().fg(Color::Yellow)),
      )
      .style(Style::default().fg(Color::White));

      message.render(overlay_area, buf);
    }
  }

  Ok(())
}

/// rat-salsa event function - handle events
pub fn event(
  event: &AppEvent,
  state: &mut AppState,
  _ctx: &mut Global,
) -> Result<Control<AppEvent>, Error> {
  // Poll shell events (temporary until we properly use PollShell)
  while let Ok(shell_event) = state.shell.event_rx.try_recv() {
    match shell_event {
      ShellEvent::Output => {
        // Shell produced output, will trigger render
      }
      ShellEvent::TermReply(reply) => {
        state.shell.writer.write_all(reply.as_bytes())?;
        state.shell.writer.flush()?;
      }
      ShellEvent::Exited(code) => {
        log::info!("Shell exited with code: {}", code);
        return Ok(Control::Quit);
      }
    }
  }

  // Poll crossterm events manually (Phase 1-2 workaround for version conflicts)
  // TODO: Phase 3+: Use PollCrossterm properly
  while event::poll(Duration::from_millis(0))? {
    match event::read()? {
      Event::Key(KeyEvent {
        code,
        modifiers,
        kind: crossterm::event::KeyEventKind::Press,
        ..
      }) => {
        // Check for hotkeys
        if matches!(
          (code, modifiers),
          (KeyCode::Char(' '), KeyModifiers::CONTROL)
        ) {
          // Ctrl-Space: toggle AI overlay
          state.ai_visible = !state.ai_visible;
          log::info!("AI overlay toggled: {}", state.ai_visible);
          return Ok(Control::Changed);
        } else if matches!(code, KeyCode::Esc) && state.ai_visible {
          // ESC: close AI overlay
          state.ai_visible = false;
          return Ok(Control::Changed);
        } else if !state.ai_visible {
          // Route to shell when AI overlay not visible
          let key = Key::new(code, modifiers);
          state.shell.send_key(key)?;
        } else {
          // AI overlay is visible - handle focus navigation and input
          // Handle Tab/Shift-Tab for focus cycling (Phase 5)
          if matches!(code, KeyCode::Tab) {
            if modifiers.contains(KeyModifiers::SHIFT) {
              // Shift-Tab: previous focus
              match_focus!(
                state.focus_input => { state.focus_conversation.focus(); },
                state.focus_conversation => { state.focus_input.focus(); }
              );
            } else {
              // Tab: next focus
              match_focus!(
                state.focus_conversation => { state.focus_input.focus(); },
                state.focus_input => { state.focus_conversation.focus(); }
              );
            }
            return Ok(Control::Changed);
          }

          // Route events based on focus
          if state.focus_conversation.get() {
            // Conversation is focused - handle scrolling
            if matches!(code, KeyCode::Up) && state.ai_process.is_some() {
              if let Some(ref mut ai_process) = state.ai_process {
                ai_process.scroll_up(1);
              }
              return Ok(Control::Changed);
            } else if matches!(code, KeyCode::Down)
              && state.ai_process.is_some()
            {
              if let Some(ref mut ai_process) = state.ai_process {
                ai_process.scroll_down(1);
              }
              return Ok(Control::Changed);
            }
          } else if state.focus_input.get() {
            // Input is focused - route to input widget
            let key = Key::new(code, modifiers);
            state.ai_ui.input_event(key);
            return Ok(Control::Changed);
          }
        }
      }
      Event::Resize(cols, rows) => {
        state.shell.resize(rows, cols)?;
        return Ok(Control::Changed);
      }
      _ => {}
    }
  }

  match event {
    AppEvent::Rendered => {
      // Rebuild focus after render (Phase 5)
      if state.ai_visible {
        let mut builder = FocusBuilder::default();
        builder.widget(&state.focus_conversation);
        builder.widget(&state.focus_input);
        builder.build();
      }
      Ok(Control::Continue)
    }
    // Shell events are handled inline above
    AppEvent::ShellOutput
    | AppEvent::ShellTermReply(_)
    | AppEvent::ShellExited(_) => Ok(Control::Continue),
  }
}

/// rat-salsa error function - handle errors
pub fn error(
  error: Error,
  _state: &mut AppState,
  _ctx: &mut Global,
) -> Result<Control<AppEvent>, Error> {
  log::error!("Error: {:?}", error);
  Ok(Control::Quit)
}

impl<'a> Drop for AppState<'a> {
  fn drop(&mut self) {
    // Cleanup terminal
    let _ = disable_raw_mode();
  }
}
