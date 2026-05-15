#![allow(warnings)]
#![allow(clippy::all, clippy::cargo, clippy::nursery, clippy::pedantic)]

// Termin.AI - Clean terminal wrapper with AI overlay

use anyhow::{Error, Result};
use clap::Parser;
use crokey::{Combiner, KeyCombination};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::terminal::disable_raw_mode;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tui::Frame;

use tui::{
  layout::Rect,
  style::{Color, Style},
  widgets::{
    Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation,
    ScrollbarState, StatefulWidget, Widget,
  },
};

// rat-salsa imports
use rat_event::Outcome;
use rat_salsa::{
  Control, RunConfig, SalsaAppContext, SalsaContext,
  poll::{PollCrossterm, PollEvents, PollRendered, PollTimers, PollTokio},
  run_tui,
  timer::{TimeOut, TimerDef},
};
use rat_theme4::{create_salsa_theme, theme::SalsaTheme};

// Import only what we need from the crate
use termin::agent_launcher::{AgentLaunchContext, build_launch_plan};
use termin::agent_terminal::AgentTerminal;
use termin::agent_tools::PendingCommand;
use termin::key::Key;
use termin::mcp_host::{
  McpServerHandle, TerminaiMcpState, start_http_mcp_server,
};
use termin::mouse::MouseEvent;
use termin::scrollback::{ScrollbackTracker, process_scrollback};
use termin::terminai_config::{ChatPosition, TerminAIConfig};

use termin::shell::{Shell, ShellEvent, ShellSpawnOptions};

const RENDER_INTERVAL: Duration = Duration::from_millis(16);

fn overlay_height_for_rows(rows: u16) -> u16 {
  (rows / 2).max(10)
}

fn agent_pty_size(rows: u16, cols: u16) -> (u16, u16) {
  (
    overlay_height_for_rows(rows).saturating_sub(2).max(1),
    cols.saturating_sub(2).max(1),
  )
}

fn render_terminal_history<R: termin::vt100::TermReplySender>(
  screen: &termin::vt100::Screen<R>,
  row_offset: usize,
  area: Rect,
  buf: &mut tui::buffer::Buffer,
) {
  for (row_idx, row) in screen
    .all_rows()
    .skip(row_offset)
    .take(area.height as usize)
    .enumerate()
  {
    for col in 0..area.width {
      if let Some(buf_cell) =
        buf.cell_mut((area.x + col, area.y + row_idx as u16))
      {
        if let Some(cell) = row.get(col) {
          *buf_cell = cell.to_tui();
          if !cell.has_contents() {
            buf_cell.set_char(' ');
          }
        } else {
          buf_cell.set_char(' ');
        }
      }
    }
  }
}

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
  AgentOutput,
  AgentTermReply(String),
  AgentExited(i32),
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

pub struct PollAgent {
  receiver: std::sync::Arc<
    std::sync::Mutex<Option<mpsc::UnboundedReceiver<ShellEvent>>>,
  >,
  cached_event: std::sync::Arc<std::sync::Mutex<Option<ShellEvent>>>,
}

impl PollAgent {
  pub fn new(receiver: mpsc::UnboundedReceiver<ShellEvent>) -> Self {
    Self {
      receiver: std::sync::Arc::new(std::sync::Mutex::new(Some(receiver))),
      cached_event: std::sync::Arc::new(std::sync::Mutex::new(None)),
    }
  }
}

impl PollEvents<AppEvent, Error> for PollAgent {
  fn as_any(&self) -> &dyn std::any::Any {
    self
  }

  fn poll(&mut self) -> Result<bool, Error> {
    if self.cached_event.lock().unwrap().is_some() {
      return Ok(true);
    }

    if let Some(ref mut rx) = *self.receiver.lock().unwrap() {
      match rx.try_recv() {
        Ok(event) => {
          *self.cached_event.lock().unwrap() = Some(event);
          Ok(true)
        }
        Err(mpsc::error::TryRecvError::Empty) => Ok(false),
        Err(mpsc::error::TryRecvError::Disconnected) => Ok(false),
      }
    } else {
      Ok(false)
    }
  }

  fn read(&mut self) -> Result<Control<AppEvent>, Error> {
    if let Some(event) = self.cached_event.lock().unwrap().take() {
      match event {
        ShellEvent::Output => Ok(Control::Event(AppEvent::AgentOutput)),
        ShellEvent::TermReply(reply) => {
          Ok(Control::Event(AppEvent::AgentTermReply(reply.to_string())))
        }
        ShellEvent::Exited(code) => {
          Ok(Control::Event(AppEvent::AgentExited(code as i32)))
        }
      }
    } else {
      Ok(Control::Continue)
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

// Use TerminalWidget from ui_layer module
use termin::ui_layer::TerminalWidget;

/// Helper to initialize shell and AI process asynchronously
async fn initialize_app_components(
  command: Vec<String>,
) -> Result<(
  Shell,
  UnboundedReceiver<ShellEvent>,
  Option<AgentTerminal>,
  UnboundedReceiver<ShellEvent>,
  Option<McpServerHandle>,
  UnboundedReceiver<PendingCommand>,
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
    Shell::spawn_command(cmd, args, rows, cols)?
  };

  let (suggestion_tx, suggestion_rx) = mpsc::unbounded_channel();
  let (agent, agent_rx, mcp_server, config, chat_position, config_error) =
    initialize_agent(&shell, suggestion_tx, rows, cols).await;

  Ok((
    shell,
    shell_event_rx,
    agent,
    agent_rx,
    mcp_server,
    suggestion_rx,
    config,
    chat_position,
    config_error,
  ))
}

/// Initialize external AI CLI terminal.
async fn initialize_agent(
  shell: &Shell,
  suggestion_tx: mpsc::UnboundedSender<PendingCommand>,
  rows: u16,
  cols: u16,
) -> (
  Option<AgentTerminal>,
  UnboundedReceiver<ShellEvent>,
  Option<McpServerHandle>,
  TerminAIConfig,
  ChatPosition,
  Option<String>,
) {
  let (fallback_tx, fallback_rx) = mpsc::unbounded_channel();
  drop(fallback_tx);

  match TerminAIConfig::load() {
    Ok(config) => {
      log::info!("Configuration loaded successfully");
      log::debug!("Loaded config: {:?}", config);
      let chat_position = config.interface.chat_position;

      let mcp_state =
        TerminaiMcpState::new(Arc::clone(&shell.vt), suggestion_tx);
      let mcp = match start_http_mcp_server(mcp_state).await {
        Ok(server) => server,
        Err(err) => {
          let message = format!("Failed to start Termin.AI MCP server: {err}");
          log::error!("{}", message);
          return (
            None,
            fallback_rx,
            None,
            config,
            chat_position,
            Some(message),
          );
        }
      };
      log::info!("Termin.AI MCP server listening at {}", mcp.url);
      let mcp_url = mcp.url.clone();

      let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());
      let launch_context = AgentLaunchContext::new(cwd.clone(), mcp_url);

      let plan = match build_launch_plan(&config.agent, &launch_context) {
        Ok(plan) => plan,
        Err(err) => {
          let message = format!("Failed to build AI CLI launch plan: {err}");
          log::error!("{}", message);
          return (
            None,
            fallback_rx,
            Some(mcp),
            config,
            chat_position,
            Some(message),
          );
        }
      };

      let available = which::which(&plan.command).is_ok();
      if !available {
        let message =
          format!("Configured AI CLI '{}' was not found in PATH", plan.command);
        log::warn!("{}", message);
        return (
          None,
          fallback_rx,
          Some(mcp),
          config,
          chat_position,
          Some(message),
        );
      }

      let (agent_rows, agent_cols) = agent_pty_size(rows, cols);
      let options = ShellSpawnOptions {
        cwd: Some(plan.cwd),
        env: plan.env,
        scrollback_len: 4000,
      };
      match AgentTerminal::spawn(
        &plan.command,
        &plan.args,
        agent_rows,
        agent_cols,
        options,
      ) {
        Ok((agent, rx)) => {
          log::info!("AI CLI terminal started: {}", plan.command);
          return (Some(agent), rx, Some(mcp), config, chat_position, None);
        }
        Err(err) => {
          let message =
            format!("Failed to start AI CLI '{}': {err}", plan.command);
          log::error!("{}", message);
          return (
            None,
            fallback_rx,
            Some(mcp),
            config,
            chat_position,
            Some(message),
          );
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
        fallback_rx,
        None,
        TerminAIConfig::default(),
        ChatPosition::default(),
        Some(error_msg),
      );
    }
  }
}

fn main() -> Result<()> {
  // Setup logging to file with rotation
  termin::terminai_init::setup_logging()?;

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
    agent_terminal,
    agent_event_rx,
    mcp_server,
    suggestion_rx,
    config,
    chat_position,
    config_error,
  ) = tokio_rt.block_on(initialize_app_components(args.command))?;
  log::info!(
    "Shell and AI components initialized, agent_present={}, chat_position={:?}",
    agent_terminal.is_some(),
    chat_position
  );

  // Create crokey combiner for keyboard event processing
  let key_combiner = Combiner::default();

  // Create PollShell for rat-salsa event loop integration
  log::debug!("Creating PollShell for event loop");
  let poll_shell = PollShell::new(shell_event_rx);
  let poll_agent = PollAgent::new(agent_event_rx);

  // Get terminal size for initial state
  let (_, rows) = crossterm::terminal::size()?;
  log::debug!("Terminal size: rows={}", rows);

  // Create theme
  log::debug!("Creating rat-salsa theme");
  let theme = create_salsa_theme("Monochrome Dark");
  let mut global = Global::new(theme);

  // Create application state
  log::debug!("Creating application state");
  // Initialize scrollback tracker from VT100's actual total_rows
  let mut scrollback_tracker = ScrollbackTracker::new();
  if let Ok(vt) = shell.vt.read() {
    scrollback_tracker.init_from_screen(vt.screen());
    log::debug!(
      "Scrollback tracker initialized with total_rows={}",
      vt.screen().total_rows()
    );
  } else {
    // Fallback to terminal height if we can't read VT
    scrollback_tracker.init(rows as usize);
    log::warn!("Could not read VT for scrollback init, using terminal height");
  }

  let mut state = AppState {
    shell,
    agent_terminal,
    _mcp_server: mcp_server,
    agent_view: TerminalViewState::new(),
    suggestion_rx,
    pending_command: None,
    ai_visible: false,
    chat_position,
    scrollback_tracker,
    config,
    config_error,
    key_combiner,
    shell_output_pending: false,
    agent_output_pending: false,
  };

  // Run rat-salsa event loop
  log::info!("Starting rat-salsa event loop");

  // Create inline terminal (no alternate screen) for native scrollback support
  let terminal = termin::terminai_init::create_terminal()?;
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
      .poll(poll_agent)
      .poll(PollRendered)
      .poll(PollTokio::new(tokio_rt)),
  ) {
    Ok(_) => log::info!("rat-salsa event loop exited normally"),
    Err(e) => {
      log::error!("rat-salsa event loop failed: {:?}", e);
      return Err(e);
    }
  }

  log::info!("terminai exiting");
  Ok(())
}

/// Application state (previously App)
struct AppState {
  shell: Shell,
  agent_terminal: Option<AgentTerminal>,
  _mcp_server: Option<McpServerHandle>,
  agent_view: TerminalViewState,
  suggestion_rx: UnboundedReceiver<PendingCommand>,
  pending_command: Option<PendingCommand>,
  ai_visible: bool,
  /// Position of AI chat overlay (top or bottom)
  chat_position: ChatPosition,
  /// Scrollback tracker for detecting and handling scrolled content
  scrollback_tracker: ScrollbackTracker,
  /// Termin.AI configuration
  config: TerminAIConfig,
  /// Configuration error message (if config failed to load)
  config_error: Option<String>,
  /// Crokey combiner for processing keyboard events
  key_combiner: Combiner,
  /// Flag indicating shell has produced output since last render.
  /// This batches multiple shell outputs into a single render on the next timer tick.
  shell_output_pending: bool,
  agent_output_pending: bool,
}

#[derive(Debug, Clone)]
struct TerminalViewState {
  row_offset: usize,
  follow_tail: bool,
}

impl TerminalViewState {
  fn new() -> Self {
    Self {
      row_offset: 0,
      follow_tail: true,
    }
  }

  fn clamp(&mut self, total_rows: usize, viewport_rows: usize) {
    let max_offset = Self::max_offset(total_rows, viewport_rows);
    if self.follow_tail {
      self.row_offset = max_offset;
    } else {
      self.row_offset = self.row_offset.min(max_offset);
      if self.row_offset >= max_offset {
        self.follow_tail = true;
      }
    }
  }

  fn scroll_lines(
    &mut self,
    delta: isize,
    total_rows: usize,
    viewport_rows: usize,
  ) {
    let max_offset = Self::max_offset(total_rows, viewport_rows);
    let next = if delta.is_negative() {
      self.row_offset.saturating_sub(delta.unsigned_abs())
    } else {
      self.row_offset.saturating_add(delta as usize)
    }
    .min(max_offset);

    self.row_offset = next;
    self.follow_tail = self.row_offset >= max_offset;
  }

  fn max_offset(total_rows: usize, viewport_rows: usize) -> usize {
    total_rows.saturating_sub(viewport_rows.max(1))
  }
}

impl AppState {
  /// Handle approval dialog key events
  /// Returns Outcome::Changed if the key was consumed, Outcome::Continue otherwise
  fn handle_approval_dialog_key(
    &mut self,
    key_combo: KeyCombination,
  ) -> Outcome {
    if self.pending_command.is_some() {
      // Approval dialog is active - check for approve/deny keys
      if self
        .config
        .interface
        .key_bindings
        .approve
        .matches(key_combo)
      {
        log::info!("Command approved by user with key: {:?}", key_combo);
        if let Some(cmd) = self.pending_command.take() {
          log::info!("Executing approved command: {}", cmd.command);
          // Send the command to the shell
          if let Err(e) = self.shell.send_command(&cmd.command) {
            log::error!("Failed to send command to shell: {:?}", e);
          }
        }
        return Outcome::Changed;
      } else if self.config.interface.key_bindings.deny.matches(key_combo) {
        log::info!("Command rejected by user with key: {:?}", key_combo);
        self.pending_command = None;
        return Outcome::Changed;
      }

      // Any other key while approval dialog is active is consumed but ignored
      log::trace!("Key {:?} ignored (approval dialog active)", key_combo);
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
      self.agent_view.follow_tail = true;
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
          termin::vt100::MouseProtocolMode::None
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

  /// Calculate the overlay height based on terminal area
  fn overlay_height(&self, area: Rect) -> u16 {
    overlay_height_for_rows(area.height).min(area.height)
  }

  /// Calculate the row offset for the terminal when overlay is visible at bottom
  fn terminal_row_offset(&self, area: Rect) -> u16 {
    if self.ai_visible && self.chat_position == ChatPosition::Bottom {
      self.overlay_height(area)
    } else {
      0
    }
  }

  /// Calculate the overlay area based on terminal area and position config
  fn overlay_area(&self, area: Rect) -> Rect {
    let overlay_height = self.overlay_height(area);
    let overlay_y = match self.chat_position {
      ChatPosition::Bottom => area.y + area.height - overlay_height,
      ChatPosition::Top => area.y,
    };
    Rect {
      x: area.x,
      y: overlay_y,
      width: area.width,
      height: overlay_height,
    }
  }

  fn overlay_inner_area(&self, area: Rect) -> Rect {
    let overlay = self.overlay_area(area);
    Rect {
      x: overlay.x.saturating_add(1),
      y: overlay.y.saturating_add(1),
      width: overlay.width.saturating_sub(2),
      height: overlay.height.saturating_sub(2),
    }
  }

  fn point_in_rect(x: u16, y: u16, area: Rect) -> bool {
    x >= area.x
      && x < area.x.saturating_add(area.width)
      && y >= area.y
      && y < area.y.saturating_add(area.height)
  }

  fn process_agent_suggestions(&mut self) -> bool {
    let mut changed = false;
    while let Ok(suggestion) = self.suggestion_rx.try_recv() {
      log::info!(
        "AI CLI suggested shell input: {} (risk: {:?})",
        suggestion.command,
        suggestion.risk_level
      );
      self.pending_command = Some(suggestion);
      changed = true;
    }
    changed
  }

  fn agent_total_rows(&self) -> usize {
    self
      .agent_terminal
      .as_ref()
      .and_then(|agent| {
        agent
          .shell()
          .vt
          .read()
          .ok()
          .map(|vt| vt.screen().total_rows())
      })
      .unwrap_or(0)
  }

  fn scroll_agent_view(&mut self, delta: isize, viewport_rows: usize) {
    let total_rows = self.agent_total_rows();
    self
      .agent_view
      .scroll_lines(delta, total_rows, viewport_rows);
  }

  /// Handle mouse events when AI overlay is visible
  fn handle_ai_mouse_event(
    &mut self,
    mouse: &crossterm::event::MouseEvent,
    terminal_area: Rect,
  ) -> Control<AppEvent> {
    use crossterm::event::MouseEventKind;

    let overlay_area = self.overlay_area(terminal_area);
    let inner_area = self.overlay_inner_area(terminal_area);

    if matches!(
      mouse.kind,
      MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
    ) && Self::point_in_rect(mouse.column, mouse.row, overlay_area)
    {
      let delta = match mouse.kind {
        MouseEventKind::ScrollUp => -3,
        MouseEventKind::ScrollDown => 3,
        _ => 0,
      };
      self.scroll_agent_view(delta, inner_area.height as usize);
      return Control::Changed;
    }

    if !Self::point_in_rect(mouse.column, mouse.row, inner_area) {
      return Control::Continue;
    }

    if let Some(agent) = &mut self.agent_terminal {
      let mouse_event =
        MouseEvent::from_crossterm(*mouse).translate(inner_area);
      if let Err(err) = agent.shell_mut().send_mouse(mouse_event) {
        log::error!("Failed to send mouse event to AI CLI: {err:?}");
      }
    }
    Control::Continue
  }

  /// Handle mouse events when AI overlay is not visible
  fn handle_shell_mouse_event(
    &mut self,
    mouse: &crossterm::event::MouseEvent,
  ) -> Result<Control<AppEvent>> {
    use crossterm::event::MouseEventKind;

    if matches!(
      mouse.kind,
      MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
    ) {
      // Allow native terminal scrollback
      log::trace!("Passing scroll event to native terminal scrollback");
      Ok(Control::Continue)
    } else {
      // Pass other mouse events to shell
      let mouse_event = MouseEvent::from_crossterm(*mouse);
      self.shell.send_mouse(mouse_event)?;
      Ok(Control::Continue)
    }
  }
}

/// rat-salsa init function - initialize focus and state
fn init(state: &mut AppState, ctx: &mut Global) -> Result<(), Error> {
  log::debug!("init() called, ai_visible={}", state.ai_visible);

  // Start the shared render timer; output events only mark pending work.
  ctx.add_timer(TimerDef::new().timer(RENDER_INTERVAL).repeat_forever());
  log::debug!("Started 60fps render timer");

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
  let scroll_up_lines = if let Ok(vt) = state.shell.vt.read() {
    let screen = vt.screen();
    let buf = frame.buffer_mut();
    process_scrollback(&mut state.scrollback_tracker, screen, buf, area)
  } else {
    log::error!("Failed to acquire read lock on VT");
    0
  };

  // Push rendered scrollback lines to native scrollback
  if scroll_up_lines > 0 {
    log::trace!(
      "Scrolling up {} lines (pending: {})",
      scroll_up_lines,
      state.scrollback_tracker.has_pending_scrollback()
    );
    frame.set_scroll_up(scroll_up_lines);
  }

  let buf = frame.buffer_mut();

  // Calculate row offset for terminal viewport using helper
  let row_offset = state.terminal_row_offset(area);

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

  // Render AI overlay if visible
  if state.ai_visible {
    // Calculate overlay area using helper
    let overlay_area = state.overlay_area(area);
    let inner_area = state.overlay_inner_area(area);

    // Clear the overlay area to prevent terminal content from showing through
    Clear.render(overlay_area, buf);
    let block = Block::default()
      .borders(Borders::ALL)
      .title(" AI Terminal ")
      .style(Style::default().fg(Color::Cyan).bg(Color::Black));
    block.render(overlay_area, buf);

    if let Some(ref agent) = state.agent_terminal {
      if let Ok(vt) = agent.shell().vt.read() {
        let screen = vt.screen();
        let total_rows = screen.total_rows();
        state
          .agent_view
          .clamp(total_rows, inner_area.height as usize);
        render_terminal_history(
          screen,
          state.agent_view.row_offset,
          inner_area,
          buf,
        );

        if total_rows > inner_area.height as usize {
          let mut scrollbar_state = ScrollbarState::new(total_rows)
            .position(state.agent_view.row_offset)
            .viewport_content_length(inner_area.height as usize);
          Scrollbar::new(ScrollbarOrientation::VerticalRight).render(
            overlay_area,
            buf,
            &mut scrollbar_state,
          );
        }

        if !screen.hide_cursor() {
          let cursor = screen.cursor_position();
          let absolute_cursor_row = screen.row0() + cursor.0 as usize;
          let viewport_end =
            state.agent_view.row_offset + inner_area.height as usize;
          if absolute_cursor_row >= state.agent_view.row_offset
            && absolute_cursor_row < viewport_end
          {
            ctx.set_screen_cursor(Some((
              inner_area.x + cursor.1.min(inner_area.width.saturating_sub(1)),
              inner_area.y
                + (absolute_cursor_row - state.agent_view.row_offset) as u16,
            )));
          } else {
            ctx.set_screen_cursor(None);
          }
        } else {
          ctx.set_screen_cursor(None);
        }
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

      let message =
        Paragraph::new(error_text).style(Style::default().fg(Color::White));

      message.render(inner_area, buf);
      ctx.set_screen_cursor(None);
    }

    if let Some(pending) = &state.pending_command {
      let approval_area = Rect {
        x: overlay_area.x,
        y: overlay_area.y,
        width: overlay_area.width,
        height: overlay_area.height.min(7),
      };
      Clear.render(approval_area, buf);
      let message = format!(
        "The AI suggests shell input:\n\n{}\n\n{}  Approve? (Y/N)",
        pending.command,
        pending
          .explanation
          .as_deref()
          .unwrap_or("No explanation provided.")
      );
      Paragraph::new(message)
        .block(
          Block::default()
            .borders(Borders::ALL)
            .title(" Shell Input Approval ")
            .style(Style::default().fg(Color::Yellow)),
        )
        .style(Style::default().fg(Color::White))
        .render(approval_area, buf);
      ctx.set_screen_cursor(None);
    }
  }

  Ok(())
}

/// rat-salsa event function - handle events
fn event(
  event: &AppEvent,
  state: &mut AppState,
  _ctx: &mut Global,
) -> Result<Control<AppEvent>, Error> {
  // Track if any state changed requiring re-render
  let mut shell_changed = state.process_agent_suggestions();

  // Check for VT scrollback changes
  if let Ok(vt) = state.shell.vt.read() {
    let screen = vt.screen();
    if screen.total_rows() > state.scrollback_tracker.last_total_rows() {
      shell_changed = true;
    }
  } else {
    log::warn!("Failed to get lock on VT")
  }

  let result = match event {
    AppEvent::Crossterm(Event::Key(
      key_event @ KeyEvent {
        code,
        modifiers,
        kind,
        ..
      },
    )) => 'm: {
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
          if state.pending_command.is_some() {
            log::debug!("Dismissing approval dialog before closing overlay");
            state.pending_command = None;
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
      if let Some(key_combo) = key_combo
        && matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat)
      {
        // Handle approval dialog with highest priority (when pending command exists)
        match state.handle_approval_dialog_key(key_combo) {
          Outcome::Changed => break 'm Control::Changed,
          Outcome::Unchanged => break 'm Control::Continue,
          Outcome::Continue => {}
        }
      }

      if matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat)
        && matches!(code, KeyCode::PageUp | KeyCode::PageDown)
      {
        let (cols, rows) = crossterm::terminal::size()?;
        let terminal_area = Rect::new(0, 0, cols, rows);
        let viewport_rows =
          state.overlay_inner_area(terminal_area).height as usize;
        let page = viewport_rows.saturating_sub(1).max(1) as isize;
        let delta = if matches!(code, KeyCode::PageUp) {
          -page
        } else {
          page
        };
        state.scroll_agent_view(delta, viewport_rows);
        break 'm Control::Changed;
      }

      if matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat)
        && let Some(agent) = &mut state.agent_terminal
      {
        let key = Key::new(*code, *modifiers);
        agent.shell_mut().send_key(key)?;
        return Ok(Control::Continue);
      }
      return Ok(if shell_changed {
        Control::Changed
      } else {
        Control::Continue
      });
    }
    AppEvent::Crossterm(Event::Resize(cols, rows)) => {
      log::info!("Terminal resize event: {}x{}", cols, rows);
      state.shell.resize(*rows, *cols)?;

      if let Some(agent) = &mut state.agent_terminal {
        let (agent_rows, agent_cols) = agent_pty_size(*rows, *cols);
        agent.shell_mut().resize(agent_rows, agent_cols)?;
      }

      // Re-synchronize scrollback tracker with VT100's new state after resize.
      // Resize can cause total_rows to change (lines wrap/unwrap), so the tracker
      // must be updated to prevent incorrect scrollback detection.
      if let Ok(vt) = state.shell.vt.read() {
        let new_total = vt.screen().total_rows();
        let old_total = state.scrollback_tracker.last_total_rows();
        state.scrollback_tracker.init_from_screen(vt.screen());
        log::debug!(
          "Scrollback tracker re-synced after resize: {} -> {}",
          old_total,
          new_total
        );
      }

      Control::Changed
    }
    AppEvent::Crossterm(Event::Mouse(mouse)) => {
      if state.ai_visible {
        let (cols, rows) = crossterm::terminal::size()?;
        state.handle_ai_mouse_event(mouse, Rect::new(0, 0, cols, rows))
      } else {
        state.handle_shell_mouse_event(mouse)?
      }
    }
    AppEvent::Crossterm(Event::Paste(text)) => {
      if !state.ai_visible {
        // Send pasted text to shell, with bracketed paste if the shell wants it
        state.shell.send_paste(text)?;
      } else if let Some(agent) = &mut state.agent_terminal {
        agent.shell_mut().send_paste(text)?;
      }
      Control::Continue
    }
    AppEvent::Crossterm(_) => {
      // Ignore other crossterm events (focus, etc.) for now
      Control::Continue
    }
    AppEvent::Timer(_) => {
      if state.shell_output_pending || state.agent_output_pending {
        state.shell_output_pending = false;
        state.agent_output_pending = false;
        Control::Changed
      } else {
        Control::Continue
      }
    }
    AppEvent::Rendered => Control::Continue,
    // Shell events now arrive via PollShell
    AppEvent::ShellOutput => {
      // Shell produced output - set flag for batched rendering on next timer tick.
      log::trace!("Shell output event - marking pending");
      state.shell_output_pending = true;
      Control::Continue
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
    AppEvent::AgentOutput => {
      state.agent_output_pending = true;
      Control::Continue
    }
    AppEvent::AgentTermReply(reply) => {
      if let Some(agent) = &mut state.agent_terminal {
        agent.shell_mut().writer.write_all(reply.as_bytes())?;
        agent.shell_mut().writer.flush()?;
      }
      Control::Continue
    }
    AppEvent::AgentExited(code) => {
      log::info!("AI CLI exited with code: {}", code);
      Control::Changed
    }
  };
  Ok(
    if shell_changed
      && result == Control::Continue
      && !matches!(event, AppEvent::AgentOutput | AppEvent::AgentTermReply(_))
    {
      Control::Changed
    } else {
      result
    },
  )
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

impl Drop for AppState {
  fn drop(&mut self) {
    // Cleanup terminal
    let _ = disable_raw_mode();
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn agent_pty_size_matches_inner_overlay_area() {
    assert_eq!(agent_pty_size(40, 120), (18, 118));
    assert_eq!(agent_pty_size(8, 1), (8, 1));
  }

  #[test]
  fn terminal_view_scrolls_and_resumes_following_tail() {
    let mut view = TerminalViewState::new();

    view.clamp(100, 20);
    assert_eq!(view.row_offset, 80);
    assert!(view.follow_tail);

    view.scroll_lines(-10, 100, 20);
    assert_eq!(view.row_offset, 70);
    assert!(!view.follow_tail);

    view.clamp(110, 20);
    assert_eq!(view.row_offset, 70);
    assert!(!view.follow_tail);

    view.scroll_lines(100, 110, 20);
    assert_eq!(view.row_offset, 90);
    assert!(view.follow_tail);
  }
}
