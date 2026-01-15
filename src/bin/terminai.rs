// Termin.AI - Clean terminal wrapper with AI overlay

use anyhow::{Error, Result};
use clap::Parser;
use crokey::{Combiner, KeyCombination};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::terminal::disable_raw_mode;
use std::io::Write;
use std::sync::Arc;
use termin::llm::ToolExecutionEvent;
use tokio::sync::{
  Mutex,
  mpsc::{self, UnboundedReceiver},
};
use tui::Frame;
use tui::style::Modifier;

use tui::{
  layout::Rect,
  style::{Color, Style},
  widgets::{Block, Borders, Clear, Paragraph, Widget},
};

// rat-salsa imports
use rat_cursor::HasScreenCursor;
use rat_event::{HandleEvent, Outcome, Regular, event_flow};
use rat_focus::FocusBuilder;
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
use termin::llm::TerminalContext;
use termin::mouse::MouseEvent;
use termin::terminai_config::{ChatPosition, TerminAIConfig};
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
  _theme: SalsaTheme,
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
      _theme: theme,
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
  row_offset: u16,
}

impl<'a> TerminalWidget<'a> {
  fn with_offset(
    screen: &'a vt100::Screen<termin::shell::ReplySender>,
    row_offset: u16,
  ) -> Self {
    Self { screen, row_offset }
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
          // Apply row offset to shift viewport (for AI overlay)
          if let Some(cell) = self.screen.cell(row + self.row_offset, col) {
            // Convert VT100 cell to tui cell (using mprocs' conversion)
            *to_cell = cell.to_tui();
            if !cell.has_contents() {
              to_cell.set_char(' ');
            }
          } else {
            // Out of bounds (offset pushed us past screen size)
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
) -> Result<(
  Shell,
  UnboundedReceiver<ShellEvent>,
  Option<AIChatProcess>,
  TerminAIConfig,
  ChatPosition,
  Option<String>,
)> {
  // Get terminal size
  let (cols, rows) = crossterm::terminal::size()?;

  // Spawn shell or command (returns Shell and event receiver)
  let (shell, shell_event_rx) = if command.is_empty() {
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

  // Initialize AI and get chat position from config
  let (ai_process, config, chat_position, config_error) = initialize_ai().await;

  Ok((
    shell,
    shell_event_rx,
    ai_process,
    config,
    chat_position,
    config_error,
  ))
}

/// Initialize AI process (extracted from App::new)
/// Returns (ai_process, config, chat_position, config_error)
async fn initialize_ai() -> (
  Option<AIChatProcess>,
  TerminAIConfig,
  ChatPosition,
  Option<String>,
) {
  match TerminAIConfig::load() {
    Ok(config) => {
      log::info!("Configuration loaded successfully");
      log::debug!("Loaded config: {:?}", config);
      let chat_position = config.interface.chat_position;

      match config.get_default_provider_and_model() {
        Ok((provider_config, model_config)) => {
          log::info!(
            "Using configured provider: {} with model: {}",
            provider_config.name,
            model_config.name
          );

          let api_key_env = provider_config.effective_api_key_env();

          // Check if API key is required and available
          let can_initialize = match api_key_env {
            None => {
              // No API key needed (e.g., Ollama running locally)
              log::info!(
                "Provider {} does not require an API key",
                provider_config.name
              );
              true
            }
            Some(ref env_key) => {
              // API key required - check if it's set
              if std::env::var(env_key).is_ok() {
                log::info!("API key {} found in environment", env_key);
                true
              } else {
                log::warn!("API key environment variable {} not set", env_key);
                false
              }
            }
          };

          if can_initialize {
            // Pass provider and model to the AI chat process
            // The subprocess will receive these via environment variables
            match AIChatProcess::new_with_provider(
              provider_config.name.clone(),
              model_config.model.clone(),
            )
            .await
            {
              Ok(process) => {
                log::info!("AI assistant initialized successfully");
                return (Some(process), config, chat_position, None);
              }
              Err(e) => {
                log::error!("Failed to initialize AI subprocess: {:?}", e);
                eprintln!("\n⚠️  Failed to initialize AI subprocess:");
                eprintln!("{}", e);
                let log_dir = xdg::BaseDirectories::with_prefix("terminai")
                  .get_cache_home()
                  .map(|path| path.to_string_lossy().to_string())
                  .unwrap_or_else(|| {
                    std::env::temp_dir()
                      .join("terminai")
                      .to_string_lossy()
                      .to_string()
                  });
                let log_path = format!("{}/terminai.log", log_dir);
                eprintln!("\n📝 Check detailed logs at: {}", log_path);
                eprintln!(
                  "   You can also set RUST_LOG=debug for verbose output.\n"
                );
                // Return config even if AI init failed
                return (None, config, chat_position, None);
              }
            }
          }
        }
        Err(e) => {
          log::error!(
            "Failed to get default provider/model from config: {:?}",
            e
          );
          // Return config even if provider/model setup failed
          return (None, config, chat_position, None);
        }
      }
    }
    Err(e) => {
      let error_msg = format!("{:#}", e);
      log::error!(
        "Failed to load configuration file: {}. AI overlay will show config instructions",
        error_msg
      );
      return (
        None,
        TerminAIConfig::default(),
        ChatPosition::default(),
        Some(error_msg),
      );
    }
  }

  (
    None,
    TerminAIConfig::default(),
    ChatPosition::default(),
    None,
  )
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
  let (
    shell,
    shell_event_rx,
    mut ai_process,
    config,
    chat_position,
    config_error,
  ) = tokio_rt.block_on(initialize_app_components(args.command))?;
  log::info!(
    "Shell and AI components initialized, ai_present={}, chat_position={:?}",
    ai_process.is_some(),
    chat_position
  );

  // Create crokey combiner for keyboard event processing
  let key_combiner = Combiner::default();

  // Create PollShell for rat-salsa event loop integration
  log::debug!("Creating PollShell for event loop");
  let poll_shell = PollShell::new(shell_event_rx);

  // Set the VT parser for scrollback reading (if AI is enabled)
  if let Some(ref mut ai_proc) = ai_process {
    log::debug!("Setting VT parser for AI scrollback reading");
    let vt_clone = Arc::clone(&shell.vt);
    tokio_rt.block_on(async {
      ai_proc.set_vt_parser(vt_clone).await;
    });
    log::info!("VT parser set for AI process");
  }

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
    chat_position,
    last_total_rows: rows as usize,
    has_pending_scrollback: false,
    config,
    config_error,
    key_combiner,
  };

  // Run rat-salsa event loop
  // Phase 2: PollShell properly integrated via rat-salsa framework
  log::info!("Starting rat-salsa event loop");
  log::debug!("Creating RunConfig with inline terminal");

  // Create inline terminal (no alternate screen) for native scrollback support
  // IMPORTANT: Disable mouse capture to allow native terminal scrolling
  // If the guest process requests mouse events, we'll pass them through
  use crossterm::cursor::SetCursorStyle;
  use crossterm::event::KeyboardEnhancementFlags;
  use rat_salsa::terminal::{CrosstermTerminal, SalsaOptions};
  use tui::TerminalOptions;
  use tui::Viewport;
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
  let terminal = CrosstermTerminal::with_options(options)?;
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
      .poll(poll_shell) // Phase 2: PollShell integrated into rat-salsa framework
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
  /// Position of AI chat overlay (top or bottom)
  chat_position: ChatPosition,
  /// Track the total row count to detect when content scrolls off screen
  last_total_rows: usize,
  /// Has pending scrollback lines:
  has_pending_scrollback: bool,
  /// Termin.AI configuration
  config: TerminAIConfig,
  /// Configuration error message (if config failed to load)
  config_error: Option<String>,
  /// Crokey combiner for processing keyboard events
  key_combiner: Combiner,
}

impl<'a> AppState<'a> {
  /// Handle approval dialog key events
  /// Returns Outcome::Changed if the key was consumed, Outcome::Continue otherwise
  fn handle_approval_dialog_key(
    &mut self,
    key_combo: KeyCombination,
  ) -> Outcome {
    if let Some(ref ai_process_arc) = self.ai_process
      && let Ok(mut ai_process) = ai_process_arc.try_lock()
    {
      if ai_process.pending_command().is_none() {
        return Outcome::Continue;
      }

      // Approval dialog is active - check for approve/deny keys
      if self
        .config
        .interface
        .key_bindings
        .approve
        .matches(key_combo)
      {
        log::info!("Command approved by user with key: {:?}", key_combo);
        if let Some(cmd) = ai_process.approve_command() {
          log::info!("Executing approved command: {}", cmd.command);
          // Send the command to the shell
          if let Err(e) = self.shell.send_command(&cmd.command) {
            log::error!("Failed to send command to shell: {:?}", e);
          }
        }
        return Outcome::Changed;
      } else if self.config.interface.key_bindings.deny.matches(key_combo) {
        log::info!("Command rejected by user with key: {:?}", key_combo);
        ai_process.reject_command();
        return Outcome::Changed;
      }

      // Any other key while approval dialog is active is consumed but ignored
      log::trace!("Key {:?} ignored (approval dialog active)", key_combo);
      return Outcome::Unchanged;
    }
    Outcome::Continue
  }

  /// Handle error dialog key events
  /// Returns Outcome::Changed if the key was consumed, Outcome::Continue otherwise
  fn handle_error_dialog_key(
    &mut self,
    key_combo: KeyCombination,
    code: KeyCode,
  ) -> Outcome {
    if let Some(ref ai_process_arc) = self.ai_process
      && let Ok(mut ai_process) = ai_process_arc.try_lock()
    {
      if ai_process.error_message().is_none() {
        return Outcome::Continue;
      }

      // Error dialog uses deactivate key to close, arrow keys to scroll
      if self
        .config
        .interface
        .key_bindings
        .deactivate_overlay
        .matches(key_combo)
      {
        log::info!("Error dialog dismissed by user with key: {:?}", key_combo);
        ai_process.clear_error();
        return Outcome::Changed;
      } else if matches!(code, KeyCode::Up) {
        ai_process.error_scroll_up(1);
        return Outcome::Changed;
      } else if matches!(code, KeyCode::Down) {
        ai_process.error_scroll_down(1);
        return Outcome::Changed;
      }

      // Any other key while error dialog is active is consumed but ignored
      log::trace!("Key {:?} ignored (error dialog active)", key_combo);
      return Outcome::Unchanged;
    }
    Outcome::Continue
  }

  /// Show the AI modal and enable mouse tracking
  fn show_ai_modal(&mut self) -> std::io::Result<()> {
    if !self.ai_visible {
      // Always enable mouse capture when showing AI modal (for our UI to handle mouse events)
      use crossterm::ExecutableCommand;
      use crossterm::event::EnableMouseCapture;
      use std::io::stdout;
      stdout().execute(EnableMouseCapture)?;
      log::debug!("Enabled mouse tracking for AI modal");

      self.ai_visible = true;
    }
    Ok(())
  }

  /// Hide the AI modal and disable mouse tracking only if guest doesn't have it enabled
  fn hide_ai_modal(&mut self) -> std::io::Result<()> {
    if self.ai_visible {
      self.ai_visible = false;

      // Check CURRENT guest mouse state (may have changed while modal was shown)
      let guest_has_mouse = if let Ok(parser) = self.shell.vt.read() {
        let screen = parser.screen();
        !matches!(
          screen.mouse_protocol_mode(),
          crate::vt100::MouseProtocolMode::None
        )
      } else {
        false
      };

      // Only disable mouse tracking if guest doesn't currently have it enabled
      if !guest_has_mouse {
        use crossterm::ExecutableCommand;
        use crossterm::event::DisableMouseCapture;
        use std::io::stdout;
        stdout().execute(DisableMouseCapture)?;
        log::debug!(
          "Disabled mouse tracking after hiding AI modal (guest doesn't have it)"
        );
      } else {
        log::debug!("Keeping mouse tracking enabled (guest has it enabled)");
      }
    }
    Ok(())
  }

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
    let cwd = std::env::current_dir()
      .unwrap_or_else(|_| PathBuf::from("/"))
      .to_string_lossy()
      .to_string();

    // No exit code tracking yet (future enhancement)
    // Note: Privacy filtering will be applied by AIChatProcess.start_streaming
    TerminalContext {
      history_lines,
      cwd,
      last_exit_code: None,
      os_info: Some(TerminalContext::get_os_info()),
      shell: TerminalContext::get_shell(),
    }
  }

  // OLD METHODS REMOVED - now using rat-salsa init/render/event functions instead
  // See init(), render(), event(), error() functions below
}

/// rat-salsa init function - initialize focus and state
fn init(state: &mut AppState, ctx: &mut Global) -> Result<(), Error> {
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
  builder.widget(state.ai_ui.conversation_state()); // Clipper state has built-in container focus
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
fn render(
  area: Rect,
  frame: &mut Frame,
  state: &mut AppState,
  ctx: &mut Global,
) -> Result<(), Error> {
  log::trace!(
    "render() called, area={:?}, ai_visible={}",
    area,
    state.ai_visible
  );

  // Detect when content has scrolled in the VT100 terminal
  // and push scrolled lines to the host terminal's native scrollback
  let num_pending_lines = if let Ok(vt) = state.shell.vt.read() {
    let screen = vt.screen();
    let current_total_rows = screen.total_rows();

    // If total rows increased, content has scrolled into VT100's scrollback buffer
    if current_total_rows > state.last_total_rows {
      let num = current_total_rows - state.last_total_rows;
      log::trace!(
        "Content scrolled: {} new lines (total rows: {} -> {})",
        num,
        state.last_total_rows,
        current_total_rows
      );
      num
    } else {
      0
    }
  } else {
    log::error!("Failed to acquire read lock on VT");
    0
  };
  let rows_to_scroll = num_pending_lines.min(area.height as usize);
  state.last_total_rows += rows_to_scroll;
  state.has_pending_scrollback = rows_to_scroll < num_pending_lines;

  // If content has scrolled, render the scrolled lines BEFORE rendering current screen
  if num_pending_lines > 0 {
    if let Ok(vt) = state.shell.vt.read() {
      let screen = vt.screen();
      let current_row0 = screen.row0();

      // The lines that just scrolled off are now in scrollback
      // They are at indices: (current_row0 - num_scrolled_lines) through (current_row0 - 1)
      let scrollback_start = current_row0.saturating_sub(num_pending_lines);

      log::trace!(
        "Inserting {} lines into scrollback of possible {}",
        rows_to_scroll,
        num_pending_lines
      );

      let mut line_idx = 0;
      for row in screen
        .all_rows()
        .skip(scrollback_start)
        .take(rows_to_scroll)
      {
        // Render this scrollback row into the buffer
        for col in 0..area.width.min(row.cols()) {
          if let Some(cell) = row.get(col) {
            if let Some(buf_cell) = frame
              .buffer_mut()
              .cell_mut((area.x + col, area.y + line_idx as u16))
            {
              *buf_cell = cell.to_tui();
              if !cell.has_contents() {
                buf_cell.modifier |= Modifier::EMPTY;
              }
            }
          }
        }
        line_idx += 1;
      }
    } else {
      log::error!("Failed to acquire read lock on VT for scrollback");
    }

    // Push these rendered lines to native scrollback
    frame.set_scroll_up(num_pending_lines as u16);
  }
  let buf = frame.buffer_mut();

  // Calculate row offset for terminal viewport
  // Only shift upwards when AI overlay is visible on the bottom
  let row_offset =
    if state.ai_visible && state.chat_position == ChatPosition::Bottom {
      (area.height / 2).max(10) // Same as overlay height
    } else {
      0
    };

  // Render current shell terminal (always visible as background)
  if let Ok(vt) = state.shell.vt.read() {
    let screen = vt.screen();
    let widget = TerminalWidget::with_offset(screen, row_offset);
    widget.render(area, buf);
    log::trace!("Shell terminal rendered with row_offset={}", row_offset);

    // Set cursor position if AI not visible and cursor should be shown
    // This matches the old code's cursor handling
    if !state.ai_visible && !screen.hide_cursor() {
      let cursor = screen.cursor_position();
      let cursor_pos = (area.x + cursor.1, area.y + cursor.0);
      log::trace!("Setting cursor position: {:?}", cursor_pos);
      ctx.set_screen_cursor(Some(cursor_pos));
    } else if !state.ai_visible {
      // Hide cursor when AI overlay is not visible but cursor should be hidden
      ctx.set_screen_cursor(None);
    }
    // If AI is visible, cursor will be set after rendering the AI UI (see below)
  } else {
    log::warn!("Failed to acquire shell vt read lock");
  }

  // Render AI overlay if visible (Phase 2)
  if state.ai_visible {
    // Calculate overlay area - full width, positioned based on config
    let overlay_height = (area.height / 2).max(10); // At least 10 lines
    let overlay_y = match state.chat_position {
      ChatPosition::Bottom => area.y + area.height - overlay_height,
      ChatPosition::Top => area.y,
    };
    let overlay_area = Rect {
      x: area.x,
      y: overlay_y,
      width: area.width,
      height: overlay_height,
    };

    // Clear the overlay area to prevent terminal content from showing through
    Clear.render(overlay_area, buf);

    if let Some(ref ai_process_arc) = state.ai_process {
      // Try to lock without blocking (non-blocking for render)
      if let Ok(ai_process) = ai_process_arc.try_lock() {
        // Render AI chat interface with Clipper's built-in focus
        state.ai_ui.render(&*ai_process, overlay_area, buf);

        // Show cursor in input area when it has focus
        if let Some((cx, cy)) = state.ai_ui.input_state().screen_cursor() {
          ctx.set_screen_cursor(Some((cx, cy)));
        } else {
          ctx.set_screen_cursor(None);
        }
      } else {
        // Lock is held (AI is processing) - render loading state
        let message = Paragraph::new("Processing... (AI is thinking)").block(
          Block::default()
            .borders(Borders::ALL)
            .title(" AI Assistant ")
            .style(Style::default().fg(Color::Cyan).bg(Color::Black)),
        );
        message.render(overlay_area, buf);
        ctx.set_screen_cursor(None);
      }
    } else {
      // Show "not configured" message with actual error if available
      let error_text = if let Some(ref err) = state.config_error {
        format!(
          "AI Assistant not configured.\n\n\
           Error: {}\n\n\
           Press ESC or Ctrl-Space to close this overlay.",
          err
        )
      } else {
        "AI Assistant not configured.\n\n\
         Configuration error.\n\n\
         Press ESC or Ctrl-Space to close this overlay."
          .to_string()
      };

      let message = Paragraph::new(error_text)
        .block(
          Block::default()
            .borders(Borders::ALL)
            .title(" AI Assistant ")
            .style(Style::default().fg(Color::Yellow)),
        )
        .style(Style::default().fg(Color::White));

      message.render(overlay_area, buf);
      ctx.set_screen_cursor(None);
    }
  }

  Ok(())
}

/// rat-salsa event function - handle events
fn event(
  event: &AppEvent,
  state: &mut AppState,
  ctx: &mut Global,
) -> Result<Control<AppEvent>, Error> {
  // Track if any state changed requiring re-render
  let mut shell_changed = false;

  // Process tool execution events from AI
  // Check for tool events and handle command suggestions
  if let Some(ref ai_process_arc) = state.ai_process {
    // Use try_lock to avoid blocking - this is a synchronous event loop
    if let Ok(mut ai_process) = ai_process_arc.try_lock() {
      // Check for tool execution events (non-blocking)
      if let Some(tool_event) = ai_process.try_recv_tool_event() {
        match tool_event {
          ToolExecutionEvent::ToolExecuted { tool_name } => {
            log::info!("Tool executed: {}", tool_name);

            // Check if it was a suggest_command tool
            if tool_name == "suggest_command" {
              log::info!("Command suggested, spawning checker task");

              // Spawn async task to retrieve and display the suggestion
              let ai_process_clone = Arc::clone(ai_process_arc);
              ctx.spawn_async(async move {
                let mut ai = ai_process_clone.lock().await;
                if let Some(suggestion) = ai.get_latest_suggestion().await {
                  log::info!(
                    "Retrieved suggestion: {} (risk: {:?})",
                    suggestion.command,
                    suggestion.risk_level
                  );
                  // Convert to pending command for approval UI
                  ai.set_pending_command(suggestion).await;
                }
                Ok(Control::Changed)
              });
            }
          }
          ToolExecutionEvent::ContinuedTextChunk { chunk } => {
            log::debug!("Continued text: {}", chunk);
            // Ensure streaming response is initialized for continued stream
            if ai_process.streaming_response().is_none() {
              ai_process.start_continued_streaming();
            }
            // Append to AI process streaming response
            ai_process.append_streaming_token(chunk);
            shell_changed = true; // Trigger re-render
          }
          ToolExecutionEvent::ContinuedStreamComplete { full_response } => {
            log::info!(
              "Continued response complete: {} chars",
              full_response.len()
            );
            // Complete the streaming with the full response
            // Use complete_continued_streaming() because tool_coordinator already
            // adds the assistant response to AG-UI history
            ai_process.complete_continued_streaming(full_response);
            shell_changed = true; // Trigger re-render
          }
          ToolExecutionEvent::Error { message } => {
            log::error!("Tool execution error: {}", message);
            ai_process.set_error(format!("Tool execution failed: {}", message));
            shell_changed = true; // Trigger re-render
          }
        }
      }
    }
  }

  if let Ok(vt) = state.shell.vt.read() {
    let screen = vt.screen();
    if screen.total_rows() > state.last_total_rows {
      shell_changed = true;
    }
  } else {
    log::warn!("Failed to get lock on VT")
  }

  let mut focus_builder = FocusBuilder::default();
  focus_builder.widget(state.ai_ui.conversation_state());
  focus_builder.widget(state.ai_ui.input_state());
  let mut focus = focus_builder.build();
  let result = match event {
    AppEvent::Crossterm(
      ct_event @ Event::Key(
        key_event @ KeyEvent {
          code,
          modifiers,
          kind,
          ..
        },
      ),
    ) => 'm: {
      // Transform KeyEvent into KeyCombination using crokey combiner
      let key_combo = state.key_combiner.transform(*key_event);
      if let Some(key_combo) = key_combo
        && matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat)
      {
        log::trace!(
          "Key event transformed: {:?} -> {:?}",
          key_event,
          key_combo
        );

        // Check for activate/deactivate overlay hotkeys (work in any mode)
        if state
          .config
          .interface
          .key_bindings
          .activate_overlay
          .matches(key_combo)
          && !state.ai_visible
        {
          log::info!("Activate overlay key pressed: {:?}", key_combo);
          state.show_ai_modal()?;
          log::info!("AI overlay shown");
          break 'm Control::Changed;
        }
        if state
          .config
          .interface
          .key_bindings
          .deactivate_overlay
          .matches(key_combo)
          && state.ai_visible
        {
          log::info!("Deactivate overlay key pressed: {:?}", key_combo);

          // Dismiss any active dialogs before closing overlay
          if let Some(ref ai_process_arc) = state.ai_process {
            if let Ok(mut ai_process) = ai_process_arc.try_lock() {
              if ai_process.error_message().is_some() {
                log::debug!("Dismissing error dialog before closing overlay");
                ai_process.clear_error();
              }
              if ai_process.pending_command().is_some() {
                log::debug!(
                  "Dismissing approval dialog before closing overlay"
                );
                ai_process.reject_command();
              }
            }
          }

          // Always close the overlay when deactivate key is pressed
          state.hide_ai_modal()?;
          log::info!("AI overlay closed");
          break 'm Control::Changed;
        }
      }
      if !state.ai_visible {
        // TODO: Kitty enhanced keyboard capability mode support?
        if matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat) {
          // Route to shell when AI overlay not visible
          let key = Key::new(*code, *modifiers);
          state.shell.send_key(key)?;
        }
        break 'm Control::Continue;
      }
      // Try handling focus events first (for tab navigation, etc.)
      if let AppEvent::Crossterm(cte) = event {
        event_flow!(break 'm focus.handle(cte, Regular));
      }

      if let Some(key_combo) = key_combo
        && matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat)
      {
        // Handle approval dialog with highest priority (when pending command exists)
        event_flow!(break 'm state.handle_approval_dialog_key(key_combo));

        // Handle error dialog (second priority after approval dialog)
        event_flow!(break 'm state.handle_error_dialog_key(key_combo, *code));
      }

      // Route events based on focus
      log::trace!(
        "Key event with AI visible - conversation focused: {}, input focused: {}, key: {:?}",
        state.ai_ui.conversation_focus().get(),
        state.ai_ui.input_focus().get(),
        code
      );

      if state.ai_ui.conversation_focus().get() {
        // Conversation is focused - use Clipper's built-in event handler
        log::debug!(
          "Conversation is focused, dispatching to Clipper event handler: {:?}",
          code
        );
        let outcome = HandleEvent::handle(
          state.ai_ui.conversation_state(),
          &Event::Key(KeyEvent::new(*code, *modifiers)),
          Regular,
        );

        return Ok(match outcome {
          Outcome::Changed => Control::Changed,
          _ => {
            if shell_changed {
              Control::Changed
            } else {
              Control::Continue
            }
          }
        });
      } else if state.ai_ui.input_focus().get() {
        // Input is focused
        // Handle Enter key to send message
        if matches!(code, KeyCode::Enter)
          && modifiers.is_empty()
          && matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat)
        {
          log::debug!("Enter pressed - sending message");
          let input = state.ai_ui.get_input_value().trim().to_string();

          if !input.is_empty() {
            log::info!("Sending message to AI: {}", input);

            // Extract terminal context before spawning task
            let context = state.extract_context();

            // Spawn async task to send message
            if let Some(ref ai_process_arc) = state.ai_process {
              let ai_process_clone = Arc::clone(ai_process_arc);
              let input_clone = input.clone();

              // Use spawn_async_ext to get a sender for intermediate render triggers
              ctx.spawn_async_ext(|sender| async move {
                use futures::stream::StreamExt;

                // Start streaming (lock only for setup)
                let stream = {
                  let mut ai_process = ai_process_clone.lock().await;
                  ai_process.start_streaming(&input_clone, context).await
                };

                match stream {
                  Ok(response) => {
                    let mut stream = response.text_stream;
                    let mut full_response = String::new();

                    // Process stream tokens with lock/unlock cycles
                    while let Some(token_result) = stream.next().await {
                      match token_result {
                        Ok(token) => {
                          full_response.push_str(&token);
                          // Lock only to update state
                          {
                            let mut ai_process = ai_process_clone.lock().await;
                            ai_process.append_streaming_token(token);
                          }
                          // Trigger UI re-render after appending token
                          let _ = sender.send(Ok(Control::Changed)).await;
                        }
                        Err(e) => {
                          let error_msg = format!("{:#}", e);
                          log::error!("Stream error: {}", error_msg);
                          let mut ai_process = ai_process_clone.lock().await;
                          ai_process.abort_streaming();
                          ai_process.set_error(error_msg);
                          return Ok(Control::Changed);
                        }
                      }
                    }

                    // Complete streaming
                    {
                      let mut ai_process = ai_process_clone.lock().await;
                      ai_process.complete_streaming(full_response).await;
                    }
                  }
                  Err(e) => {
                    let error_msg = format!("{:#}", e);
                    log::error!("Failed to start streaming: {}", error_msg);
                    let mut ai_process = ai_process_clone.lock().await;
                    ai_process.abort_streaming();
                    ai_process.set_error(error_msg);
                  }
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
        log::trace!("Routing key to input widget");
        state.ai_ui.input_event(ct_event);
        return Ok(Control::Changed);
      }

      log::warn!("No widget has focus! Input should be focused by default");
      return Ok(if shell_changed {
        Control::Changed
      } else {
        Control::Continue
      });
    }
    AppEvent::Crossterm(Event::Resize(cols, rows)) => {
      state.shell.resize(*rows, *cols)?;
      Control::Changed
    }
    AppEvent::Crossterm(Event::Mouse(mouse)) => {
      use crossterm::event::MouseEventKind;

      if state.ai_visible {
        // AI modal is visible - use Clipper's built-in mouse handler
        // Clipper handles scroll events, dragging, etc. automatically
        let outcome = HandleEvent::handle(
          state.ai_ui.conversation_state(),
          &Event::Mouse(*mouse),
          rat_event::MouseOnly,
        );

        match outcome {
          Outcome::Changed => Control::Changed,
          _ => Control::Continue,
        }
      } else {
        // AI modal not visible
        if matches!(
          mouse.kind,
          MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
        ) {
          // Allow native terminal scrollback
          log::trace!("Passing scroll event to native terminal scrollback");
          Control::Continue
        } else {
          // Pass other mouse events to shell
          let mouse_event = MouseEvent::from_crossterm(*mouse);
          state.shell.send_mouse(mouse_event)?;
          Control::Continue
        }
      }
    }
    AppEvent::Crossterm(_) => {
      // Ignore other crossterm events (mouse, focus, paste, etc.) for now
      Control::Continue
    }
    AppEvent::Timer(_) => {
      // Periodic timer (60fps) - trigger render if shell has changed
      // This ensures we render at 60fps like the old code did
      Control::Continue
    }
    AppEvent::Rendered => {
      // Rebuild focus after render to track widget positions
      if state.ai_visible {
        let mut builder = FocusBuilder::default();
        builder.widget(state.ai_ui.conversation_state());
        builder.widget(state.ai_ui.input_state()); // Use widget's built-in focus
        let focus = builder.build();
        ctx.set_focus(focus);
      }
      Control::Continue
    }
    // Shell events now arrive via PollShell
    AppEvent::ShellOutput => {
      // Shell produced output, trigger re-render
      log::trace!("Shell output event");
      Control::Changed
    }
    AppEvent::ShellTermReply(reply) => {
      // Write terminal reply back to shell
      state.shell.writer.write_all(reply.as_bytes())?;
      state.shell.writer.flush()?;
      log::trace!("Shell term reply sent");
      Control::Continue
    }
    AppEvent::ShellExited(code) => {
      log::info!("Shell exited with code: {}", code);
      Control::Quit
    }
  };
  Ok(if shell_changed && result == Control::Continue {
    Control::Changed
  } else {
    result
  })
}

/// rat-salsa error function - handle errors
fn error(
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
