// Termin.AI - Clean single-shell terminal with AI overlay
// Uses only the minimal PTY/VT100 code from mprocs, no UI chrome

use anyhow::{Error, Result};
use clap::Parser;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::disable_raw_mode;
use std::io::Write;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

use tui::{
  layout::{Constraint, Direction, Layout, Rect},
  style::{Color, Style},
  widgets::{Block, Borders, Paragraph, Widget},
};

// rat-salsa imports
use rat_focus::{FocusBuilder, match_focus};
use rat_salsa::{
  Control, RunConfig, SalsaAppContext, SalsaContext,
  poll::{PollCrossterm, PollEvents, PollRendered, PollTimers, PollTokio},
  run_tui,
  timer::{TimeOut, TimerDef},
};
use rat_theme4::{create_salsa_theme, theme::SalsaTheme};

// Import only what we need from the crate
use termin::ai_proc::{AIChatProcess, AIChatUI};
use termin::key::Key;
use termin::llm::{Provider, TerminalContext};
use termin::mouse::MouseEvent;
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

  /// Timer event (for periodic rendering)
  Timer(TimeOut),

  /// Crossterm event (keyboard, mouse, resize, etc.)
  Crossterm(Event),

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

impl From<TimeOut> for AppEvent {
  fn from(timeout: TimeOut) -> Self {
    Self::Timer(timeout)
  }
}

impl From<Event> for AppEvent {
  fn from(event: Event) -> Self {
    Self::Crossterm(event)
  }
}

/// Custom event source for shell events
pub struct PollShell {
  receiver: Arc<std::sync::Mutex<Option<mpsc::UnboundedReceiver<ShellEvent>>>>,
  cached_event: Arc<std::sync::Mutex<Option<ShellEvent>>>,
}

impl PollShell {
  pub fn new(receiver: mpsc::UnboundedReceiver<ShellEvent>) -> Self {
    Self {
      receiver: Arc::new(std::sync::Mutex::new(Some(receiver))),
      cached_event: Arc::new(std::sync::Mutex::new(None)),
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

fn main() -> Result<()> {
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

  // Create tokio runtime for async operations
  // NOTE: PollTokio requires manual runtime initialization (cannot use #[tokio::main])
  log::debug!("Creating tokio runtime");
  let tokio_rt = tokio::runtime::Runtime::new()?;

  // Initialize shell and AI asynchronously
  log::debug!("Initializing shell and AI components");
  let (shell, ai_process) =
    tokio_rt.block_on(initialize_app_components(args.command))?;
  log::info!(
    "Shell and AI components initialized, ai_present={}",
    ai_process.is_some()
  );

  // Get terminal size for initial state
  let (_, rows) = crossterm::terminal::size()?;
  log::debug!("Terminal size: rows={}", rows);

  // Create theme
  log::debug!("Creating rat-salsa theme");
  let theme = create_salsa_theme("Monochrome Dark");
  let mut global = Global::new(theme);

  // Create application state
  log::debug!("Creating application state");
  let mut state = AppState {
    shell,
    ai_process: ai_process.map(|p| Arc::new(Mutex::new(p))),
    ai_ui: AIChatUI::new(),
    ai_visible: false,
    last_total_rows: rows as usize,
    focus_conversation: rat_focus::FocusFlag::default(),
  };

  // Run rat-salsa event loop
  // NOTE: For Phase 1, we poll crossterm events manually to avoid version conflicts
  // NOTE: Shell events are also polled inline in the event() function
  // TODO: Phase 2+: Use PollCrossterm and PollShell properly
  log::info!("Starting rat-salsa event loop");
  log::debug!("Creating RunConfig with inline terminal");

  // Create inline terminal (no alternate screen) for native scrollback support
  // This matches the old code's Viewport::Inline behavior
  use rat_salsa::terminal::CrosstermTerminal;
  let (_, rows) = crossterm::terminal::size()?;
  let terminal = CrosstermTerminal::inline(rows, false)?;
  let config = RunConfig::<AppEvent, Error>::new(terminal);
  log::debug!("Calling run_tui");
  match run_tui(
    init,
    render,
    event,
    error,
    &mut global,
    &mut state,
    config
      .poll(PollTimers::default())
      .poll(PollCrossterm)
      .poll(PollRendered)
      .poll(PollTokio::new(tokio_rt)),
  ) {
    Ok(_) => log::info!("rat-salsa event loop exited normally"),
    Err(e) => {
      log::error!("rat-salsa event loop failed: {:?}", e);
      return Err(e.into());
    }
  }

  log::info!("terminai exiting");
  Ok(())
}

/// Application state (previously App)
struct AppState<'a> {
  shell: Shell,
  ai_process: Option<Arc<Mutex<AIChatProcess>>>,
  ai_ui: AIChatUI<'a>,
  ai_visible: bool,
  /// Track the total row count to detect when content scrolls off screen
  last_total_rows: usize,
  /// Focus for conversation area (no widget for this, so we manage it separately)
  focus_conversation: rat_focus::FocusFlag,
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

/// rat-salsa init function - initialize focus and state
pub fn init(state: &mut AppState, ctx: &mut Global) -> Result<(), Error> {
  log::debug!("init() called, ai_visible={}", state.ai_visible);

  // Start 60fps timer for periodic rendering (like the old code)
  ctx.add_timer(
    TimerDef::new()
      .timer(std::time::Duration::from_millis(16)) // 60fps
      .repeat_forever(),
  );
  log::debug!("Started 60fps render timer");

  // Initialize focus (Phase 5)
  // Always build focus, but it's only active when AI modal is visible
  log::debug!("Building focus for AI modal");
  let mut builder = FocusBuilder::default();
  builder.widget(&state.focus_conversation);
  builder.widget(state.ai_ui.input_state()); // Use widget's built-in focus
  let focus = builder.build();
  // Focus on input by default
  focus.focus(state.ai_ui.input_focus());
  ctx.set_focus(focus);
  log::debug!("Focus initialized and set in context, input focused");
  log::debug!("init() completed");
  Ok(())
}

/// rat-salsa render function - render the UI
pub fn render(
  area: Rect,
  buf: &mut tui::buffer::Buffer,
  state: &mut AppState,
  ctx: &mut Global,
) -> Result<(), Error> {
  log::debug!(
    "render() called, area={:?}, ai_visible={}",
    area,
    state.ai_visible
  );
  // Render shell terminal (always visible as background)
  if let Ok(vt) = state.shell.vt.read() {
    let screen = vt.screen();
    let widget = TerminalWidget::new(screen);
    widget.render(area, buf);
    log::trace!("Shell terminal rendered");

    // Set cursor position if AI not visible and cursor should be shown
    // This matches the old code's cursor handling
    if !state.ai_visible && !screen.hide_cursor() {
      let cursor = screen.cursor_position();
      let cursor_pos = (area.x + cursor.1, area.y + cursor.0);
      log::trace!("Setting cursor position: {:?}", cursor_pos);
      ctx.set_screen_cursor(Some(cursor_pos));
    } else {
      // Hide cursor when AI overlay is visible
      ctx.set_screen_cursor(None);
    }
  } else {
    log::warn!("Failed to acquire shell vt read lock");
  }

  // Render AI overlay if visible (Phase 2)
  if state.ai_visible {
    // Calculate overlay area - full width, bottom 50% of screen
    let overlay_height = (area.height / 2).max(10); // At least 10 lines
    let overlay_area = Rect {
      x: area.x,
      y: area.y + area.height - overlay_height,
      width: area.width,
      height: overlay_height,
    };

    if let Some(ref ai_process_arc) = state.ai_process {
      // Try to lock without blocking (non-blocking for render)
      if let Ok(ai_process) = ai_process_arc.try_lock() {
        // Render AI chat interface with focus flags (Phase 5)
        state.ai_ui.render(
          &*ai_process,
          overlay_area,
          buf,
          &state.focus_conversation,
        );
      } else {
        // Lock is held (AI is processing) - render loading state
        let message = Paragraph::new("Processing... (AI is thinking)").block(
          Block::default()
            .borders(Borders::ALL)
            .title(" AI Assistant ")
            .style(Style::default().fg(Color::Cyan).bg(Color::Black)),
        );
        message.render(overlay_area, buf);
      }
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
  ctx: &mut Global,
) -> Result<Control<AppEvent>, Error> {
  // Process shell events FIRST (on every event handler call)
  // This ensures shell output is processed even when we return early for keyboard events
  // Limit to 5 events per iteration to stay responsive
  let mut shell_changed = false;
  for _ in 0..5 {
    match state.shell.event_rx.try_recv() {
      Ok(shell_event) => {
        match shell_event {
          ShellEvent::Output => {
            // Shell produced output, need to re-render
            shell_changed = true;
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
      Err(_) => break, // No more events
    }
  }

  // Now process keyboard events (high priority)
  if let AppEvent::Crossterm(Event::Key(KeyEvent {
    code,
    modifiers,
    kind: crossterm::event::KeyEventKind::Press,
    ..
  })) = event
  {
    // Check for hotkeys
    if matches!(
      (*code, *modifiers),
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
      let key = Key::new(*code, *modifiers);
      state.shell.send_key(key)?;
      // Return Changed if shell output was pending, otherwise Continue
      // This triggers render for the shell output
      return Ok(if shell_changed {
        Control::Changed
      } else {
        Control::Continue
      });
    } else {
      // AI overlay is visible - handle focus navigation and input
      // Handle Tab/Shift-Tab for focus cycling (Phase 5)
      if matches!(code, KeyCode::Tab) {
        if modifiers.contains(KeyModifiers::SHIFT) {
          // Shift-Tab: previous focus
          match_focus!(
            state.ai_ui.input_focus() => { state.focus_conversation.focus(); },
            state.focus_conversation => { state.ai_ui.input_focus().focus(); }
          );
        } else {
          // Tab: next focus
          match_focus!(
            state.focus_conversation => { state.ai_ui.input_focus().focus(); },
            state.ai_ui.input_focus() => { state.focus_conversation.focus(); }
          );
        }
        return Ok(Control::Changed);
      }

      // Handle approval dialog with highest priority (when pending command exists)
      if let Some(ref ai_process_arc) = state.ai_process {
        if let Ok(mut ai_process) = ai_process_arc.try_lock() {
          if ai_process.pending_command().is_some() {
            // Approval dialog is active - handle 'y' or 'n'
            if matches!(code, KeyCode::Char('y' | 'Y')) && modifiers.is_empty()
            {
              log::info!("Command approved by user");
              if let Some(cmd) = ai_process.approve_command() {
                log::info!("Executing approved command: {}", cmd.command);
                // Send the command to the shell
                if let Err(e) = state.shell.send_command(&cmd.command) {
                  log::error!("Failed to send command to shell: {:?}", e);
                }
              }
              return Ok(Control::Changed);
            } else if matches!(code, KeyCode::Char('n' | 'N'))
              && modifiers.is_empty()
            {
              log::info!("Command rejected by user");
              ai_process.reject_command();
              return Ok(Control::Changed);
            }
            // Any other key while approval dialog is active is ignored
            return Ok(Control::Continue);
          }
        }
      }

      // Route events based on focus
      log::debug!(
        "Key event with AI visible - conversation focused: {}, input focused: {}, key: {:?}",
        state.focus_conversation.get(),
        state.ai_ui.input_focus().get(),
        code
      );

      if state.focus_conversation.get() {
        // Conversation is focused - handle scrolling
        if matches!(code, KeyCode::Up) && state.ai_process.is_some() {
          if let Some(ref ai_process_arc) = state.ai_process {
            if let Ok(mut ai_process) = ai_process_arc.try_lock() {
              ai_process.scroll_up(1);
            }
          }
          return Ok(Control::Changed);
        } else if matches!(code, KeyCode::Down) && state.ai_process.is_some() {
          if let Some(ref ai_process_arc) = state.ai_process {
            if let Ok(mut ai_process) = ai_process_arc.try_lock() {
              ai_process.scroll_down(1);
            }
          }
          return Ok(Control::Changed);
        }
        // Conversation is read-only, ignore other keys
        return Ok(Control::Continue);
      } else if state.ai_ui.input_focus().get() {
        // Input is focused
        // Handle Enter key to send message
        if matches!(code, KeyCode::Enter) && modifiers.is_empty() {
          log::debug!("Enter pressed - sending message");
          let input = state.ai_ui.get_input_value();

          if !input.is_empty() {
            log::info!("Sending message to AI: {}", input);

            // Extract terminal context before spawning task
            let context = state.extract_context();

            // Spawn async task to send message
            if let Some(ref ai_process_arc) = state.ai_process {
              let ai_process_clone = Arc::clone(ai_process_arc);
              let input_clone = input.clone();

              ctx.spawn_async(async move {
                // Async lock (safe to hold across await in async context)
                let mut ai_process = ai_process_clone.lock().await;
                if let Err(e) = ai_process
                  .send_input_with_context(&input_clone, context)
                  .await
                {
                  log::error!("Failed to send message: {:?}", e);
                }
                Ok(Control::Changed)
              });
            }

            // Clear input after queuing send
            state.ai_ui.clear_input();
          }
          return Ok(Control::Changed);
        }

        // Route other keys to input widget
        log::debug!("Routing key to input widget");
        let key = Key::new(*code, *modifiers);
        state.ai_ui.input_event(key);
        return Ok(Control::Changed);
      }

      log::warn!("No widget has focus! Input should be focused by default");
      return Ok(if shell_changed {
        Control::Changed
      } else {
        Control::Continue
      });
    }
  }

  // If shell output changed, trigger a render
  if shell_changed {
    return Ok(Control::Changed);
  }

  match event {
    AppEvent::Crossterm(Event::Resize(cols, rows)) => {
      state.shell.resize(*rows, *cols)?;
      Ok(Control::Changed)
    }
    AppEvent::Crossterm(Event::Mouse(mouse)) => {
      use crossterm::event::MouseEventKind;

      // Filter out scroll events to allow native terminal scrollback
      // This matches the old code's behavior
      if matches!(
        mouse.kind,
        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
      ) {
        // Don't consume scroll events - let the terminal handle them for native scrollback
        log::trace!("Ignoring scroll event for native scrollback");
        return Ok(Control::Continue);
      }

      // Convert crossterm mouse event to our MouseEvent type
      let mouse_event = MouseEvent::from_crossterm(*mouse);

      if state.ai_visible {
        // AI overlay is visible - handle mouse for UI interaction
        // TODO: Implement focus changes based on click position
        // For now, just consume the event without action
        Ok(Control::Continue)
      } else {
        // AI overlay not visible - pass through to shell
        state.shell.send_mouse(mouse_event)?;
        Ok(Control::Continue)
      }
    }
    // Ignore other key events (already handled above)
    AppEvent::Crossterm(Event::Key(KeyEvent {
      code,
      modifiers,
      kind: crossterm::event::KeyEventKind::Press,
      ..
    })) => {
      // Check for hotkeys
      if matches!(
        (*code, *modifiers),
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
        let key = Key::new(*code, *modifiers);
        state.shell.send_key(key)?;
      } else {
        // AI overlay is visible - handle focus navigation and input
        // Handle Tab/Shift-Tab for focus cycling (Phase 5)
        if matches!(code, KeyCode::Tab) {
          if modifiers.contains(KeyModifiers::SHIFT) {
            // Shift-Tab: previous focus
            match_focus!(
              state.ai_ui.input_focus() => { state.focus_conversation.focus(); },
              state.focus_conversation => { state.ai_ui.input_focus().focus(); }
            );
          } else {
            // Tab: next focus
            match_focus!(
              state.focus_conversation => { state.ai_ui.input_focus().focus(); },
              state.ai_ui.input_focus() => { state.focus_conversation.focus(); }
            );
          }
          return Ok(Control::Changed);
        }

        // Route events based on focus
        if state.focus_conversation.get() {
          // Conversation is focused - handle scrolling
          if matches!(code, KeyCode::Up) && state.ai_process.is_some() {
            if let Some(ref ai_process_arc) = state.ai_process {
              if let Ok(mut ai_process) = ai_process_arc.try_lock() {
                ai_process.scroll_up(1);
              }
            }
            return Ok(Control::Changed);
          } else if matches!(code, KeyCode::Down) && state.ai_process.is_some()
          {
            if let Some(ref ai_process_arc) = state.ai_process {
              if let Ok(mut ai_process) = ai_process_arc.try_lock() {
                ai_process.scroll_down(1);
              }
            }
            return Ok(Control::Changed);
          }
        } else if state.ai_ui.input_focus().get() {
          // Input is focused
          // Handle Enter key to send message
          if matches!(code, KeyCode::Enter) && modifiers.is_empty() {
            log::debug!("Enter pressed - sending message");
            let input = state.ai_ui.get_input_value();

            if !input.is_empty() {
              log::info!("Sending message to AI: {}", input);

              // Extract terminal context before spawning task
              let context = state.extract_context();

              // Spawn async task to send message
              if let Some(ref ai_process_arc) = state.ai_process {
                let ai_process_clone = Arc::clone(ai_process_arc);
                let input_clone = input.clone();

                ctx.spawn_async(async move {
                  // Async lock (safe to hold across await in async context)
                  let mut ai_process = ai_process_clone.lock().await;
                  if let Err(e) = ai_process
                    .send_input_with_context(&input_clone, context)
                    .await
                  {
                    log::error!("Failed to send message: {:?}", e);
                  }
                  Ok(Control::Changed)
                });
              }

              // Clear input after queuing send
              state.ai_ui.clear_input();
            }
            return Ok(Control::Changed);
          }

          // Route other keys to input widget
          let key = Key::new(*code, *modifiers);
          state.ai_ui.input_event(key);
          return Ok(Control::Changed);
        }
      }
      Ok(Control::Continue)
    }
    AppEvent::Crossterm(Event::Resize(cols, rows)) => {
      state.shell.resize(*rows, *cols)?;
      Ok(Control::Changed)
    }
    AppEvent::Crossterm(_) => {
      // Ignore other crossterm events (mouse, focus, paste, etc.) for now
      Ok(Control::Continue)
    }
    AppEvent::Timer(_) => {
      // Periodic timer (60fps) - trigger render if shell has changed
      // This ensures we render at 60fps like the old code did
      if shell_changed {
        Ok(Control::Changed)
      } else {
        Ok(Control::Continue)
      }
    }
    AppEvent::Rendered => {
      // Rebuild focus after render to track widget positions
      if state.ai_visible {
        let mut builder = FocusBuilder::default();
        builder.widget(&state.focus_conversation);
        builder.widget(state.ai_ui.input_state()); // Use widget's built-in focus
        let focus = builder.build();
        ctx.set_focus(focus);
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
