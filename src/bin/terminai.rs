#![allow(warnings)]
#![allow(clippy::all, clippy::cargo, clippy::nursery, clippy::pedantic)]

// Terminai - Clean terminal wrapper with AI overlay

use anyhow::{Context, Error, Result};
use base64::Engine;
use clap::{Parser, Subcommand};
use crokey::{Combiner, KeyCombination};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::{
  cursor::MoveTo,
  execute,
  terminal::{Clear as TerminalClear, ClearType, disable_raw_mode},
};
use notify::{
  Event as NotifyEvent, EventKind as NotifyEventKind, RecommendedWatcher,
  RecursiveMode, Watcher,
};
use rmcp::{
  ServiceExt,
  model::CallToolRequestParams,
  transport::{
    StreamableHttpClientTransport,
    streamable_http_client::StreamableHttpClientTransportConfig,
  },
};
use std::collections::VecDeque;
use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc as std_mpsc;
use std::time::Duration;
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tui::Frame;

use tui::{
  layout::Rect,
  style::{Color, Modifier, Style},
  text::{Line, Span},
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
use termin::agent_launcher::{
  AgentLaunchContext, AgentLaunchPlan, available_agent_presets,
  build_launch_plan,
};
use termin::agent_terminal::AgentTerminal;
use termin::agent_tools::PendingCommand;
use termin::key::Key;
use termin::mcp_host::tool_defs::{
  CHECK_FOR_UPDATES, GET_SUGGESTION_STATUS, GET_TERMINAL_CONTEXT,
  READ_TERMINAL, ReadTerminalArgs, SUGGEST_INPUT, SuggestInputArgs,
};
use termin::mcp_host::{
  McpServerHandle, TerminaiMcpState, run_stdio_mcp_proxy, start_http_mcp_server,
};
use termin::mouse::MouseEvent;
use termin::privacy::PrivacyFilter;
use termin::scrollback::{
  ScrollbackTracker, drain_pending_native_scrollback_snapshot,
};
use termin::terminai_config::{
  AgentConfig, ApprovalMode, ChatPosition, TerminaiConfig,
};
use termin::ui_approval::{
  ApprovalAction, approval_action_at, approval_content_line_count,
  approval_modal_area, approval_viewport_height, max_approval_scroll,
  render_shell_input_approval_with_state,
};
use termin::ui_controls::{
  ControlModal, ControlPanelItem, render_control_modal,
};

use termin::shell::{OutputWakeup, Shell, ShellEvent, ShellSpawnOptions};
use termin::shell_resolution::{parent_shell, resolve_shell};

const RENDER_INTERVAL: Duration = Duration::from_millis(16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SuggestionAction {
  Ask,
  AutoApprove,
}

fn suggestion_action(
  mode: ApprovalMode,
  _risk: termin::command::RiskLevel,
) -> SuggestionAction {
  match mode {
    ApprovalMode::AlwaysAsk => SuggestionAction::Ask,
    ApprovalMode::AutoApproval => SuggestionAction::AutoApprove,
  }
}

fn approval_status(mode: ApprovalMode) -> Option<Line<'static>> {
  (mode == ApprovalMode::AutoApproval).then(|| {
    Line::from(Span::styled(
      " ⚠ AUTO-APPROVE ",
      Style::default()
        .fg(Color::Yellow)
        .bg(Color::Black)
        .add_modifier(Modifier::BOLD),
    ))
    .right_aligned()
  })
}

fn startup_agent_name(config: &TerminaiConfig) -> String {
  config
    .agent
    .preset
    .clone()
    .or_else(|| config.agent.command.clone())
    .unwrap_or_else(|| "codex".to_string())
}

fn switcher_agents(config: &TerminaiConfig) -> Result<Vec<String>> {
  let mut agents = available_agent_presets(&config.agent_presets)?;
  let startup = startup_agent_name(config);
  if !agents.contains(&startup) {
    agents.push(startup);
    agents.sort();
  }
  Ok(agents)
}

fn agent_config_for_choice(
  config: &TerminaiConfig,
  choice: &str,
) -> AgentConfig {
  if startup_agent_name(config) == choice {
    return config.agent.clone();
  }
  AgentConfig {
    preset: Some(choice.to_string()),
    kind: None,
    command: None,
    args: Vec::new(),
    extra_args: Vec::new(),
    prompt_template: None,
    uses_mcp: None,
    uses_tool_cli: None,
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RecoveryPhase {
  Starting,
  Running,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RecoveryFallback {
  RequestedCommand,
  InteractiveShell,
}

fn recovery_fallback(phase: RecoveryPhase) -> RecoveryFallback {
  match phase {
    RecoveryPhase::Starting => RecoveryFallback::RequestedCommand,
    RecoveryPhase::Running => RecoveryFallback::InteractiveShell,
  }
}

#[derive(Clone, Debug)]
struct RecoveryCommand {
  command: String,
  args: Vec<String>,
}

impl RecoveryCommand {
  fn from_resolved(shell: termin::shell_resolution::ResolvedShell) -> Self {
    Self {
      command: shell.command,
      args: shell.fallback_args,
    }
  }

  fn display(&self) -> String {
    std::iter::once(self.command.as_str())
      .chain(self.args.iter().map(String::as_str))
      .collect::<Vec<_>>()
      .join(" ")
  }
}

struct RecoveryContext {
  phase: AtomicU8,
  recovering: AtomicBool,
  requested: RecoveryCommand,
  interactive: RecoveryCommand,
}

impl RecoveryContext {
  fn new(command: &[String]) -> Self {
    let config = TerminaiConfig::load().ok();
    let requested = resolve_shell(
      command,
      std::env::var("TERMINAI_SHELL").ok(),
      config.as_ref().map(|config| &config.shell),
      parent_shell(),
    )
    .map(RecoveryCommand::from_resolved)
    .unwrap_or_else(|_| RecoveryCommand {
      command: command
        .first()
        .cloned()
        .unwrap_or_else(default_shell_command),
      args: command.get(1..).unwrap_or_default().to_vec(),
    });
    let interactive = resolve_shell(&[], None, None, parent_shell())
      .map(RecoveryCommand::from_resolved)
      .unwrap_or_else(|_| RecoveryCommand {
        command: default_shell_command(),
        args: Vec::new(),
      });
    Self {
      phase: AtomicU8::new(0),
      recovering: AtomicBool::new(false),
      requested,
      interactive,
    }
  }

  fn mark_running(&self) {
    self.phase.store(1, Ordering::Release);
  }

  fn phase(&self) -> RecoveryPhase {
    match self.phase.load(Ordering::Acquire) {
      0 => RecoveryPhase::Starting,
      _ => RecoveryPhase::Running,
    }
  }

  fn recover(&self, reason: &str) -> ! {
    if self
      .recovering
      .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
      .is_err()
    {
      loop {
        std::thread::park();
      }
    }

    reset_host_terminal();
    let fallback = recovery_fallback(self.phase());
    let command = match fallback {
      RecoveryFallback::RequestedCommand => &self.requested,
      RecoveryFallback::InteractiveShell => &self.interactive,
    };
    let message = match fallback {
      RecoveryFallback::RequestedCommand => format!(
        "Terminai failed {reason}. Starting the requested command: {}",
        command.display()
      ),
      RecoveryFallback::InteractiveShell => format!(
        "Terminai failed {reason}. It would have restarted: {}. Starting an interactive fallback shell: {}",
        self.requested.display(),
        command.display()
      ),
    };
    log::error!("{message}");
    eprintln!("{message}");
    execute_recovery_command(command)
  }
}

fn reset_host_terminal() {
  let _ = disable_raw_mode();
  let mut stdout = std::io::stdout();
  // RIS (Reset to Initial State) restores terminal modes, rendition, cursor,
  // character sets, and colors before the fallback takes over the terminal.
  let _ = stdout.write_all(b"\x1bc");
  let _ = stdout.flush();
}

#[cfg(unix)]
fn default_shell_command() -> String {
  std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
}

#[cfg(windows)]
fn default_shell_command() -> String {
  "cmd.exe".to_string()
}

#[cfg(unix)]
fn execute_recovery_command(command: &RecoveryCommand) -> ! {
  use std::os::unix::process::CommandExt;
  let error = Command::new(&command.command).args(&command.args).exec();
  eprintln!("Terminai could not start fallback command: {error}");
  std::process::exit(1)
}

#[cfg(windows)]
fn execute_recovery_command(command: &RecoveryCommand) -> ! {
  match Command::new(&command.command).args(&command.args).status() {
    Ok(status) => std::process::exit(status.code().unwrap_or(1)),
    Err(error) => {
      eprintln!("Terminai could not start fallback command: {error}");
      std::process::exit(1)
    }
  }
}

fn install_panic_recovery(recovery: Arc<RecoveryContext>) {
  std::panic::set_hook(Box::new(move |info| {
    let message = format!("after a panic: {info}");
    log::error!("Terminai panic: {info}");
    let recovery = Arc::clone(&recovery);
    std::thread::spawn(move || recovery.recover(&message));
  }));
}

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
#[command(
  name = "terminai",
  author,
  version,
  about,
  long_about = None,
  args_conflicts_with_subcommands = true
)]
struct Args {
  #[command(subcommand)]
  subcommand: Option<CliCommand>,

  /// Command to run (if not specified, resolves a supported shell)
  #[arg(last = true)]
  command: Vec<String>,
}

#[derive(Subcommand, Debug)]
enum CliCommand {
  /// Create default Terminai config files in the config directory.
  InitConfig {
    /// Replace existing config files instead of leaving them untouched.
    #[arg(long)]
    force: bool,
  },

  /// Run Terminai's internal MCP stdio proxy.
  #[command(name = "_mcp", hide = true)]
  Mcp,

  /// Call Terminai MCP tools from a non-MCP CLI client.
  #[command(name = "tool", hide = true)]
  Tool {
    /// Print structured JSON instead of text content.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: ToolCommand,
  },
}

#[derive(Subcommand, Debug)]
enum ToolCommand {
  /// Read the user's wrapped terminal screen and recent scrollback.
  #[command(name = "read_terminal", alias = "read-terminal")]
  ReadTerminal {
    /// Maximum number of scrollback/screen lines to return.
    #[arg(long)]
    max_lines: Option<usize>,
    /// Include visible terminal rows in the result.
    #[arg(long)]
    include_visible: Option<bool>,
  },

  /// Check for Terminai context updates.
  #[command(name = "check_for_updates", alias = "check-for-updates")]
  CheckForUpdates,

  /// Get concise metadata about the wrapped terminal.
  #[command(name = "get_terminal_context", alias = "get-terminal-context")]
  GetTerminalContext,

  /// Suggest exact input for Terminai to offer to the user for approval.
  #[command(name = "suggest_input", alias = "suggest-input")]
  SuggestInput {
    /// Exact input to offer for approval.
    input: String,
    /// Explanation shown with the suggested input.
    #[arg(long)]
    explanation: Option<String>,
  },

  /// Return the most recent queued shell input suggestion.
  #[command(name = "get_suggestion_status", alias = "get-suggestion-status")]
  GetSuggestionStatus,
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
  ShellHostEscape(String),
  ShellExited(i32),
  AgentOutput,
  AgentTermReply(String),
  AgentExited(i32),
  ConfigChanged,
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
  cached_events: Arc<std::sync::Mutex<VecDeque<ShellEvent>>>,
}

impl PollShell {
  pub fn new(receiver: mpsc::UnboundedReceiver<ShellEvent>) -> Self {
    Self {
      receiver: Arc::new(std::sync::Mutex::new(Some(receiver))),
      cached_events: Arc::new(std::sync::Mutex::new(VecDeque::new())),
    }
  }
}

pub struct PollAgent {
  receiver: SharedAgentReceiver,
  cached_events: std::sync::Arc<std::sync::Mutex<VecDeque<ShellEvent>>>,
}

type SharedAgentReceiver =
  Arc<std::sync::Mutex<Option<mpsc::UnboundedReceiver<ShellEvent>>>>;

pub struct PollConfigWatcher {
  _watcher: Option<RecommendedWatcher>,
  receiver: std_mpsc::Receiver<()>,
  pending: bool,
}

impl PollConfigWatcher {
  pub fn new(paths: Vec<PathBuf>) -> Self {
    let (tx, rx) = std_mpsc::channel();
    let file_names: std::collections::HashSet<OsString> = paths
      .iter()
      .filter_map(|path| path.file_name().map(OsString::from))
      .collect();
    let watch_dirs: std::collections::HashSet<PathBuf> = paths
      .iter()
      .filter_map(|path| path.parent().map(PathBuf::from))
      .collect();

    let watcher = match notify::recommended_watcher(
      move |result: notify::Result<NotifyEvent>| match result {
        Ok(event) if notify_event_matches(&event, &file_names) => {
          let _ = tx.send(());
        }
        Ok(_) => {}
        Err(err) => log::warn!("Config watcher error: {err}"),
      },
    ) {
      Ok(mut watcher) => {
        for dir in watch_dirs {
          if dir.exists() {
            if let Err(err) = watcher.watch(&dir, RecursiveMode::NonRecursive) {
              log::warn!(
                "Failed to watch config directory {}: {err}",
                dir.display()
              );
            } else {
              log::info!("Watching config directory {}", dir.display());
            }
          } else {
            log::debug!(
              "Config directory does not exist yet, not watching {}",
              dir.display()
            );
          }
        }
        Some(watcher)
      }
      Err(err) => {
        log::warn!("Failed to create config watcher: {err}");
        None
      }
    };

    Self {
      _watcher: watcher,
      receiver: rx,
      pending: false,
    }
  }
}

fn notify_event_matches(
  event: &NotifyEvent,
  file_names: &std::collections::HashSet<OsString>,
) -> bool {
  if !matches!(
    event.kind,
    NotifyEventKind::Any
      | NotifyEventKind::Create(_)
      | NotifyEventKind::Modify(_)
      | NotifyEventKind::Remove(_)
      | NotifyEventKind::Other
  ) {
    return false;
  }

  event.paths.iter().any(|path| {
    path
      .file_name()
      .is_some_and(|file_name| file_names.contains(file_name))
  })
}

impl PollAgent {
  pub fn new(receiver: SharedAgentReceiver) -> Self {
    Self {
      receiver,
      cached_events: std::sync::Arc::new(
        std::sync::Mutex::new(VecDeque::new()),
      ),
    }
  }
}

impl PollEvents<AppEvent, Error> for PollConfigWatcher {
  fn as_any(&self) -> &dyn std::any::Any {
    self
  }

  fn poll(&mut self) -> Result<bool, Error> {
    if self.pending {
      return Ok(true);
    }

    match self.receiver.try_recv() {
      Ok(()) => {
        while self.receiver.try_recv().is_ok() {}
        self.pending = true;
        Ok(true)
      }
      Err(std_mpsc::TryRecvError::Empty) => Ok(false),
      Err(std_mpsc::TryRecvError::Disconnected) => Ok(false),
    }
  }

  fn read(&mut self) -> Result<Control<AppEvent>, Error> {
    if self.pending {
      self.pending = false;
      Ok(Control::Event(AppEvent::ConfigChanged))
    } else {
      Ok(Control::Continue)
    }
  }
}

impl PollEvents<AppEvent, Error> for PollAgent {
  fn as_any(&self) -> &dyn std::any::Any {
    self
  }

  fn poll(&mut self) -> Result<bool, Error> {
    if !self.cached_events.lock().unwrap().is_empty() {
      return Ok(true);
    }

    if let Some(ref mut rx) = *self.receiver.lock().unwrap() {
      match rx.try_recv() {
        Ok(event) => {
          let mut cached_events = self.cached_events.lock().unwrap();
          push_coalesced_output_event(&mut cached_events, event);
          while let Ok(event) = rx.try_recv() {
            push_coalesced_output_event(&mut cached_events, event);
            if matches!(cached_events.back(), Some(ShellEvent::Exited(_))) {
              break;
            }
          }
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
    if let Some(event) = self.cached_events.lock().unwrap().pop_front() {
      match event {
        ShellEvent::Output(wakeup) => {
          wakeup.clear();
          Ok(Control::Event(AppEvent::AgentOutput))
        }
        ShellEvent::TermReply(reply) => {
          Ok(Control::Event(AppEvent::AgentTermReply(reply.to_string())))
        }
        ShellEvent::HostEscape(_) => Ok(Control::Continue),
        ShellEvent::Exited(code) => {
          Ok(Control::Event(AppEvent::AgentExited(code as i32)))
        }
      }
    } else {
      Ok(Control::Continue)
    }
  }
}

fn push_coalesced_output_event(
  events: &mut VecDeque<ShellEvent>,
  event: ShellEvent,
) {
  if matches!(event, ShellEvent::Output(_))
    && matches!(events.back(), Some(ShellEvent::Output(_)))
  {
    return;
  }
  events.push_back(event);
}

fn push_coalesced_shell_event(
  events: &mut VecDeque<ShellEvent>,
  event: ShellEvent,
) {
  push_coalesced_output_event(events, event);
}

impl PollEvents<AppEvent, Error> for PollShell {
  fn as_any(&self) -> &dyn std::any::Any {
    self
  }

  fn poll(&mut self) -> Result<bool, Error> {
    if !self.cached_events.lock().unwrap().is_empty() {
      return Ok(true);
    }

    if let Some(ref mut rx) = *self.receiver.lock().unwrap() {
      match rx.try_recv() {
        Ok(event) => {
          let mut cached_events = self.cached_events.lock().unwrap();
          push_coalesced_shell_event(&mut cached_events, event);
          while let Ok(event) = rx.try_recv() {
            push_coalesced_shell_event(&mut cached_events, event);
            if matches!(cached_events.back(), Some(ShellEvent::Exited(_))) {
              break;
            }
          }
          Ok(true)
        }
        Err(mpsc::error::TryRecvError::Empty) => Ok(false),
        Err(mpsc::error::TryRecvError::Disconnected) => {
          self
            .cached_events
            .lock()
            .unwrap()
            .push_back(ShellEvent::Exited(1));
          Ok(true)
        }
      }
    } else {
      Ok(false)
    }
  }

  fn read(&mut self) -> Result<Control<AppEvent>, Error> {
    if let Some(event) = self.cached_events.lock().unwrap().pop_front() {
      match event {
        ShellEvent::Output(wakeup) => {
          wakeup.clear();
          Ok(Control::Event(AppEvent::ShellOutput))
        }
        ShellEvent::TermReply(reply) => {
          Ok(Control::Event(AppEvent::ShellTermReply(reply.to_string())))
        }
        ShellEvent::HostEscape(escape) => Ok(Control::Event(
          AppEvent::ShellHostEscape(escape.to_string()),
        )),
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

const DEFAULT_STARTUP_TERMINAL_SIZE: (u16, u16) = (80, 24);

fn startup_terminal_size(cols: u16, rows: u16) -> (u16, u16) {
  if cols == 0 || rows == 0 {
    DEFAULT_STARTUP_TERMINAL_SIZE
  } else {
    (cols, rows)
  }
}

/// Helper to initialize shell and prepare AI integration asynchronously
async fn initialize_app_components(
  command: Vec<String>,
) -> Result<(
  Shell,
  UnboundedReceiver<ShellEvent>,
  UnboundedReceiver<ShellEvent>,
  Option<McpServerHandle>,
  Option<TerminaiMcpState>,
  Option<AgentLaunchPlan>,
  UnboundedReceiver<PendingCommand>,
  TerminaiConfig,
  ChatPosition,
  Option<String>,
)> {
  // Get terminal size
  let (reported_cols, reported_rows) = crossterm::terminal::size()?;
  let (cols, rows) = startup_terminal_size(reported_cols, reported_rows);
  if (cols, rows) != (reported_cols, reported_rows) {
    log::warn!(
      "Terminal reported invalid startup size {}x{}; using {}x{} until the first resize event",
      reported_cols,
      reported_rows,
      cols,
      rows
    );
  }

  // Spawn shell or command (returns Shell and event receiver)
  let config_for_shell = TerminaiConfig::load().ok();
  let resolved_shell = resolve_shell(
    &command,
    std::env::var("TERMINAI_SHELL").ok(),
    config_for_shell.as_ref().map(|config| &config.shell),
    parent_shell(),
  )?;
  log::info!(
    "Spawning wrapped command: {} {:?}",
    resolved_shell.command,
    resolved_shell.args
  );
  let (shell, shell_event_rx) = Shell::spawn_command(
    &resolved_shell.command,
    &resolved_shell.args,
    rows,
    cols,
  )?;

  let (suggestion_tx, suggestion_rx) = mpsc::unbounded_channel();
  let (
    agent_rx,
    mcp_server,
    mcp_state,
    agent_launch_plan,
    config,
    chat_position,
    config_error,
  ) = prepare_agent(&shell, suggestion_tx, &resolved_shell.identity).await;

  Ok((
    shell,
    shell_event_rx,
    agent_rx,
    mcp_server,
    mcp_state,
    agent_launch_plan,
    suggestion_rx,
    config,
    chat_position,
    config_error,
  ))
}

/// Prepare external AI CLI configuration without launching the AI process.
async fn prepare_agent(
  shell: &Shell,
  suggestion_tx: mpsc::UnboundedSender<PendingCommand>,
  shell_identity: &str,
) -> (
  UnboundedReceiver<ShellEvent>,
  Option<McpServerHandle>,
  Option<TerminaiMcpState>,
  Option<AgentLaunchPlan>,
  TerminaiConfig,
  ChatPosition,
  Option<String>,
) {
  let (fallback_tx, fallback_rx) = mpsc::unbounded_channel();
  drop(fallback_tx);

  match TerminaiConfig::load() {
    Ok(config) => {
      log::info!("Configuration loaded successfully");
      log::debug!("Loaded config: {:?}", config);
      let chat_position = config.interface.chat_position;

      let privacy_filter = match PrivacyFilter::from_config(&config.privacy) {
        Ok(filter) => filter,
        Err(err) => {
          let message = format!("Invalid privacy configuration: {err}");
          log::error!("{}", message);
          return (
            fallback_rx,
            None,
            None,
            None,
            config,
            chat_position,
            Some(message),
          );
        }
      };
      let mcp_state = TerminaiMcpState::with_privacy_filter(
        Arc::clone(&shell.vt),
        suggestion_tx,
        shell_identity,
        privacy_filter,
      );
      let mcp_auth_token = match generate_mcp_auth_token() {
        Ok(token) => token,
        Err(err) => {
          let message =
            format!("Failed to generate Terminai MCP auth token: {err}");
          log::error!("{}", message);
          return (
            fallback_rx,
            None,
            None,
            None,
            config,
            chat_position,
            Some(message),
          );
        }
      };
      let mcp =
        match start_http_mcp_server(mcp_state.clone(), mcp_auth_token.clone())
          .await
        {
          Ok(server) => server,
          Err(err) => {
            let message = format!("Failed to start Terminai MCP server: {err}");
            log::error!("{}", message);
            return (
              fallback_rx,
              None,
              None,
              None,
              config,
              chat_position,
              Some(message),
            );
          }
        };
      log::info!("Terminai MCP server listening at {}", mcp.url);
      let mcp_url = mcp.url.clone();
      let mcp_port = mcp.port.to_string();

      let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());
      let launch_context = AgentLaunchContext::new(
        cwd.clone(),
        mcp_url,
        mcp_auth_token,
        terminai_binary_path(),
        mcp_port,
      );

      let mut plan = match build_launch_plan(
        &config.agent,
        &config.agent_presets,
        &launch_context,
      ) {
        Ok(plan) => plan,
        Err(err) => {
          let message = format!("Failed to build AI CLI launch plan: {err}");
          log::error!("{}", message);
          return (
            fallback_rx,
            Some(mcp),
            Some(mcp_state),
            None,
            config,
            chat_position,
            Some(message),
          );
        }
      };
      normalize_agent_launch_plan_env(&mut plan);

      return (
        fallback_rx,
        Some(mcp),
        Some(mcp_state),
        Some(plan),
        config,
        chat_position,
        None,
      );
    }
    Err(e) => {
      let error_msg = format!("{:#}", e);
      log::error!(
        "Failed to load configuration file: {}. AI overlay will show config instructions",
        error_msg
      );
      return (
        fallback_rx,
        None,
        None,
        None,
        TerminaiConfig::default(),
        ChatPosition::default(),
        Some(error_msg),
      );
    }
  }
}

fn generate_mcp_auth_token() -> Result<String> {
  let mut token = [0_u8; 32];
  getrandom::fill(&mut token).map_err(|err| {
    anyhow::anyhow!("failed to read secure random bytes: {err}")
  })?;
  Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(token))
}

fn terminai_binary_path() -> String {
  std::env::current_exe()
    .ok()
    .and_then(|path| path.into_os_string().into_string().ok())
    .unwrap_or_else(|| "terminai".to_string())
}

fn spawn_agent_from_plan(
  plan: &AgentLaunchPlan,
  rows: u16,
  cols: u16,
) -> Result<(AgentTerminal, UnboundedReceiver<ShellEvent>)> {
  let (agent_rows, agent_cols) = agent_pty_size(rows, cols);
  let options = ShellSpawnOptions {
    cwd: Some(plan.cwd.clone()),
    env: plan.env.clone(),
    scrollback_len: 4000,
  };
  AgentTerminal::spawn(
    &plan.command,
    &plan.args,
    agent_rows,
    agent_cols,
    options,
  )
}

fn modal_title_for_agent(plan: &AgentLaunchPlan) -> String {
  plan
    .command
    .rsplit(['/', '\\'])
    .next()
    .filter(|name| !name.is_empty())
    .unwrap_or("AI Terminal")
    .to_string()
}

fn normalize_agent_launch_plan_env(plan: &mut AgentLaunchPlan) {
  if let Some(path) = augmented_agent_path(plan.env.get("PATH")) {
    plan.env.insert("PATH".to_string(), path);
  }
}

fn agent_command_available(plan: &AgentLaunchPlan) -> bool {
  if PathBuf::from(&plan.command).components().count() > 1 {
    return which::which(&plan.command).is_ok();
  }

  if let Some(path) = plan.env.get("PATH") {
    which::which_in(&plan.command, Some(path), &plan.cwd).is_ok()
  } else {
    which::which(&plan.command).is_ok()
  }
}

fn augmented_agent_path(configured_path: Option<&String>) -> Option<String> {
  let base_path = configured_path
    .map(OsString::from)
    .or_else(|| std::env::var_os("PATH"))?;
  let mut paths: Vec<PathBuf> = std::env::split_paths(&base_path).collect();

  for path in common_user_bin_dirs() {
    if !paths.iter().any(|existing| existing == &path) {
      paths.push(path);
    }
  }

  std::env::join_paths(paths)
    .ok()
    .and_then(|path| path.into_string().ok())
}

fn common_user_bin_dirs() -> Vec<PathBuf> {
  let mut dirs = Vec::new();
  if let Some(home) = std::env::var_os("HOME") {
    let home = PathBuf::from(home);
    dirs.push(home.join(".local/bin"));
    dirs.push(home.join(".cargo/bin"));
  }
  dirs.push(PathBuf::from("/opt/homebrew/bin"));
  dirs.push(PathBuf::from("/usr/local/bin"));
  dirs
}

fn run_init_config(force: bool) -> Result<()> {
  let result = termin::terminai_config_init::init_config_files(force)?;
  println!(
    "Initialized Terminai config directory: {}",
    result.config_dir.display()
  );
  for file in result.files {
    let action = match file.action {
      termin::terminai_config_init::ConfigInitAction::Written => "wrote",
      termin::terminai_config_init::ConfigInitAction::Skipped => "skipped",
    };
    println!("{} {}", action, file.path.display());
  }
  Ok(())
}

async fn run_tool_command(
  command: ToolCommand,
  json_output: bool,
) -> Result<()> {
  let auth_token = std::env::var("TERMINAI_MCP_AUTH_TOKEN")
    .context("TERMINAI_MCP_AUTH_TOKEN is required for terminai tool")?;
  if auth_token.trim().is_empty() {
    anyhow::bail!("TERMINAI_MCP_AUTH_TOKEN must not be empty");
  }
  let port = std::env::var("TERMINAI_MCP_PORT")
    .context("TERMINAI_MCP_PORT is required for terminai tool")?
    .parse::<u16>()
    .context("TERMINAI_MCP_PORT must be a valid TCP port")?;
  let url = format!("http://127.0.0.1:{port}/mcp");
  let transport = StreamableHttpClientTransport::from_config(
    StreamableHttpClientTransportConfig::with_uri(url).auth_header(auth_token),
  );
  let client = ()
    .serve(transport)
    .await
    .context("failed to connect to Terminai HTTP MCP server")?;
  let result = client
    .peer()
    .call_tool(tool_request(command)?)
    .await
    .context("Terminai tool call failed")?;

  print_tool_result(&result, json_output)?;
  client
    .cancel()
    .await
    .context("failed to close Terminai MCP client")?;

  if result.is_error == Some(true) {
    anyhow::bail!("Terminai tool returned an error");
  }
  Ok(())
}

fn tool_request(command: ToolCommand) -> Result<CallToolRequestParams> {
  let (name, arguments) = match command {
    ToolCommand::ReadTerminal {
      max_lines,
      include_visible,
    } => (
      READ_TERMINAL,
      Some(tool_arguments(ReadTerminalArgs {
        max_lines,
        include_visible,
      })?),
    ),
    ToolCommand::CheckForUpdates => (CHECK_FOR_UPDATES, None),
    ToolCommand::GetTerminalContext => (GET_TERMINAL_CONTEXT, None),
    ToolCommand::SuggestInput { input, explanation } => (
      SUGGEST_INPUT,
      Some(tool_arguments(SuggestInputArgs { input, explanation })?),
    ),
    ToolCommand::GetSuggestionStatus => (GET_SUGGESTION_STATUS, None),
  };
  let request = CallToolRequestParams::new(name);
  Ok(match arguments {
    Some(arguments) => request.with_arguments(arguments),
    None => request,
  })
}

fn tool_arguments<T: serde::Serialize>(
  args: T,
) -> Result<serde_json::Map<String, serde_json::Value>> {
  match serde_json::to_value(args)? {
    serde_json::Value::Object(arguments) => Ok(arguments),
    _ => anyhow::bail!("tool arguments did not serialize to a JSON object"),
  }
}

fn print_tool_result(
  result: &rmcp::model::CallToolResult,
  json_output: bool,
) -> Result<()> {
  if json_output {
    let value = result
      .structured_content
      .clone()
      .unwrap_or_else(|| serde_json::to_value(result).unwrap_or_default());
    println!("{}", serde_json::to_string_pretty(&value)?);
    return Ok(());
  }

  let mut text_blocks = Vec::new();
  for content in &result.content {
    if let Some(text) = content.as_text() {
      text_blocks.push(text.text.as_str());
    }
  }

  if text_blocks.is_empty() {
    println!("{}", serde_json::to_string_pretty(result)?);
  } else {
    println!("{}", text_blocks.join("\n"));
  }
  Ok(())
}

fn clear_host_terminal() -> std::io::Result<()> {
  let mut stdout = std::io::stdout();
  execute!(stdout, TerminalClear(ClearType::All), MoveTo(0, 0))?;
  stdout.flush()
}

fn config_watch_paths() -> Vec<PathBuf> {
  let mut paths = Vec::new();
  if let Ok(path) = TerminaiConfig::expected_path() {
    paths.push(path);
  }
  paths.push(termin::env_loader::env_file_path());
  paths
}

fn percent_decode_path(input: &str) -> String {
  let mut output = Vec::with_capacity(input.len());
  let bytes = input.as_bytes();
  let mut idx = 0;
  while idx < bytes.len() {
    if bytes[idx] == b'%' && idx + 2 < bytes.len() {
      let hex = &input[idx + 1..idx + 3];
      if let Ok(value) = u8::from_str_radix(hex, 16) {
        output.push(value);
        idx += 3;
        continue;
      }
    }
    output.push(bytes[idx]);
    idx += 1;
  }
  String::from_utf8_lossy(&output).into_owned()
}

fn cwd_from_osc7_escape(escape: &str) -> Option<PathBuf> {
  let cwd = escape.strip_prefix("\x1b]7;")?;
  let cwd = cwd.trim_end_matches('\x07');
  let cwd = cwd.strip_suffix("\x1b\\").unwrap_or(cwd);

  let path = if let Some(uri) = cwd.strip_prefix("file://") {
    let path_start = uri.find('/').unwrap_or(uri.len());
    &uri[path_start..]
  } else {
    cwd
  };

  let path = percent_decode_path(path);
  // file:///C:/... has an extra URI slash before the drive. cmd's $P uses
  // backslashes, so normalize both forms before handing the path to the host.
  let path = if path.as_bytes().get(0) == Some(&b'/')
    && path.as_bytes().get(2) == Some(&b':')
    && path.as_bytes().get(1).is_some_and(u8::is_ascii_alphabetic)
  {
    path[1..].replace('\\', "/")
  } else {
    path.replace('\\', "/")
  };

  if path.is_empty() {
    None
  } else {
    Some(PathBuf::from(path))
  }
}

fn rebuild_agent_launch_plan_for_cwd(
  plan: &AgentLaunchPlan,
  config: &TerminaiConfig,
  cwd: PathBuf,
) -> Result<AgentLaunchPlan> {
  let launch_context = AgentLaunchContext::new(
    cwd,
    plan.metadata.mcp_url.clone(),
    plan.metadata.mcp_auth_token.clone(),
    plan.metadata.terminai_binary_path.clone(),
    plan.metadata.terminai_mcp_port.clone(),
  );
  build_launch_plan(&config.agent, &config.agent_presets, &launch_context)
}

fn main() -> Result<()> {
  let args = Args::parse();
  match args.subcommand {
    Some(CliCommand::InitConfig { force }) => return run_init_config(force),
    Some(CliCommand::Mcp) => {
      let tokio_rt = tokio::runtime::Runtime::new()?;
      return tokio_rt.block_on(run_stdio_mcp_proxy());
    }
    Some(CliCommand::Tool { json, command }) => {
      let tokio_rt = tokio::runtime::Runtime::new()?;
      return tokio_rt.block_on(run_tool_command(command, json));
    }
    None => {}
  }

  let recovery = Arc::new(RecoveryContext::new(&args.command));
  install_panic_recovery(Arc::clone(&recovery));
  let command = args.command;
  match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    run_interactive(command, Arc::clone(&recovery))
  })) {
    Ok(Ok(())) => Ok(()),
    Ok(Err(error)) => recovery.recover(&format!("with an error: {error:#}")),
    Err(_) => recovery.recover("after a panic"),
  }
}

fn run_interactive(
  command: Vec<String>,
  recovery: Arc<RecoveryContext>,
) -> Result<()> {
  // Setup logging to file with rotation
  termin::terminai_init::setup_logging()?;
  termin::terminai_init::require_windows_terminal()?;

  // Load optional user environment variables from terminai.env.
  if let Err(e) = termin::env_loader::load_env_file() {
    log::error!("Failed to load terminai.env: {}", e);
    eprintln!("Error: {}", e);
    std::process::exit(1);
  }

  log::info!("Terminai starting");
  clear_host_terminal()?;

  // Create tokio runtime for async operations
  // NOTE: PollTokio requires manual runtime initialization (cannot use #[tokio::main])
  log::debug!("Creating tokio runtime");
  let tokio_rt = tokio::runtime::Runtime::new()?;

  // Initialize shell and prepare AI integration asynchronously
  log::debug!("Initializing shell and AI components");
  let (
    shell,
    shell_event_rx,
    agent_event_rx,
    mcp_server,
    mcp_state,
    agent_launch_plan,
    suggestion_rx,
    config,
    chat_position,
    config_error,
  ) = tokio_rt.block_on(initialize_app_components(command))?;
  log::info!(
    "Shell and AI components initialized, agent_deferred=true, chat_position={:?}",
    chat_position
  );

  // Create crokey combiner for keyboard event processing
  let key_combiner = Combiner::default();

  // Create PollShell for rat-salsa event loop integration
  log::debug!("Creating PollShell for event loop");
  let poll_shell = PollShell::new(shell_event_rx);
  let agent_event_rx = Arc::new(std::sync::Mutex::new(Some(agent_event_rx)));
  let poll_agent = PollAgent::new(Arc::clone(&agent_event_rx));
  let poll_config = PollConfigWatcher::new(config_watch_paths());

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
    shell_cwd: agent_launch_plan
      .as_ref()
      .map(|plan| plan.cwd.clone())
      .or_else(|| std::env::current_dir().ok()),
    mcp_state,
    shell,
    agent_terminal: None,
    mcp_server,
    agent_launch_plan,
    active_agent_name: startup_agent_name(&config),
    active_agent_config: config.agent.clone(),
    agent_event_rx,
    agent_view: TerminalViewState::new(),
    suggestion_rx,
    pending_command: None,
    approval_mode: config.approval_mode,
    approval_scroll: 0,
    approval_focus: ApprovalAction::Approve,
    control_modal: None,
    agent_exit_status: None,
    agent_modal_title: "AI Terminal".to_string(),
    ai_visible: false,
    chat_position,
    scrollback_tracker,
    config,
    config_error,
    key_combiner,
    shell_output_pending: false,
    agent_output_pending: false,
    recovery: Some(recovery),
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
      .poll(poll_config)
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
  shell_cwd: Option<PathBuf>,
  mcp_state: Option<TerminaiMcpState>,
  shell: Shell,
  agent_terminal: Option<AgentTerminal>,
  mcp_server: Option<McpServerHandle>,
  agent_launch_plan: Option<AgentLaunchPlan>,
  active_agent_name: String,
  active_agent_config: AgentConfig,
  agent_event_rx: SharedAgentReceiver,
  agent_view: TerminalViewState,
  suggestion_rx: UnboundedReceiver<PendingCommand>,
  pending_command: Option<PendingCommand>,
  approval_mode: ApprovalMode,
  approval_scroll: usize,
  approval_focus: ApprovalAction,
  control_modal: Option<ControlModal>,
  agent_exit_status: Option<i32>,
  agent_modal_title: String,
  ai_visible: bool,
  /// Position of AI chat overlay (top or bottom)
  chat_position: ChatPosition,
  /// Scrollback tracker for detecting and handling scrolled content
  scrollback_tracker: ScrollbackTracker,
  /// Terminai configuration
  config: TerminaiConfig,
  /// Configuration error message (if config failed to load)
  config_error: Option<String>,
  /// Crokey combiner for processing keyboard events
  key_combiner: Combiner,
  /// Flag indicating shell has produced output since last render.
  /// This batches multiple shell outputs into a single render on the next timer tick.
  shell_output_pending: bool,
  agent_output_pending: bool,
  recovery: Option<Arc<RecoveryContext>>,
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
    let current = if self.follow_tail {
      max_offset
    } else {
      self.row_offset.min(max_offset)
    };
    let next = if delta.is_negative() {
      current.saturating_sub(delta.unsigned_abs())
    } else {
      current.saturating_add(delta as usize)
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
        self.run_approval_action(ApprovalAction::Approve);
        return Outcome::Changed;
      } else if self.config.interface.key_bindings.deny.matches(key_combo) {
        log::info!("Command rejected by user with key: {:?}", key_combo);
        self.run_approval_action(ApprovalAction::Deny);
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
      if self.agent_terminal.is_none()
        && self.agent_exit_status.is_none()
        && self.config_error.is_none()
      {
        let (cols, rows) = crossterm::terminal::size()?;
        self.launch_agent(rows, cols);
      }

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

  fn deactivate_ai_overlay(&mut self) -> std::io::Result<()> {
    self.hide_ai_modal()
  }

  fn max_approval_scroll(&self, area: Rect) -> usize {
    self
      .pending_command
      .as_ref()
      .map(|pending| {
        max_approval_scroll(
          approval_content_line_count(pending),
          approval_viewport_height(area),
        )
      })
      .unwrap_or(0)
  }

  fn clamp_approval_scroll(&mut self, area: Rect) {
    self.approval_scroll =
      self.approval_scroll.min(self.max_approval_scroll(area));
  }

  fn scroll_approval(&mut self, delta: isize, area: Rect) -> bool {
    let old = self.approval_scroll;
    let max_scroll = self.max_approval_scroll(area);
    let next = if delta.is_negative() {
      old.saturating_sub(delta.unsigned_abs())
    } else {
      old.saturating_add(delta as usize)
    }
    .min(max_scroll);

    self.approval_scroll = next;
    next != old
  }

  fn toggle_approval_focus(&mut self) {
    self.approval_focus = match self.approval_focus {
      ApprovalAction::Approve => ApprovalAction::Deny,
      ApprovalAction::Deny => ApprovalAction::Approve,
    };
  }

  fn activate_focused_approval(&mut self) {
    self.run_approval_action(self.approval_focus);
  }

  fn run_approval_action(&mut self, action: ApprovalAction) {
    match action {
      ApprovalAction::Approve => {
        if let Some(cmd) = self.pending_command.take() {
          log::info!("Executing approved command: {}", cmd.command);
          if let Err(e) = self.shell.send_command(&cmd.command) {
            log::error!("Failed to send command to shell: {:?}", e);
          }
          if let Err(err) = self.hide_ai_modal() {
            log::error!("Failed to hide AI modal after approval: {err:?}");
          }
        }
      }
      ApprovalAction::Deny => {
        self.pending_command = None;
      }
    }
    self.approval_scroll = 0;
    self.approval_focus = ApprovalAction::Approve;
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
      let action = suggestion_action(self.approval_mode, suggestion.risk_level);
      self.pending_command = Some(suggestion);
      self.approval_scroll = 0;
      self.approval_focus = ApprovalAction::Approve;
      if action == SuggestionAction::AutoApprove {
        self.control_modal = None;
        self.run_approval_action(ApprovalAction::Approve);
      }
      changed = true;
    }
    changed
  }

  fn request_approval_mode_toggle(&mut self) {
    if self.approval_mode == ApprovalMode::AutoApproval {
      self.approval_mode = ApprovalMode::AlwaysAsk;
      self.control_modal = None;
    } else {
      self.control_modal = Some(ControlModal::confirm_auto_approval());
    }
  }

  fn open_agent_picker(&mut self) {
    match switcher_agents(&self.config) {
      Ok(agents) => {
        let selected = agents
          .iter()
          .position(|name| name == &self.active_agent_name)
          .unwrap_or(0);
        let mut modal = ControlModal::agent_picker(agents);
        for _ in 0..selected {
          modal.next();
        }
        self.control_modal = Some(modal);
      }
      Err(err) => {
        self.config_error =
          Some(format!("Failed to list configured AI agents: {err}"))
      }
    }
  }

  fn clear_internal_history(&mut self) -> Result<()> {
    let mut vt =
      self.shell.vt.write().map_err(|_| {
        anyhow::anyhow!("shell terminal state lock is poisoned")
      })?;
    vt.clear_scrollback();
    self.scrollback_tracker.init_from_screen(vt.screen());
    Ok(())
  }

  fn build_runtime_agent_plan(
    &self,
    agent: &AgentConfig,
  ) -> Result<AgentLaunchPlan> {
    let cwd = self
      .shell_cwd
      .clone()
      .or_else(|| std::env::current_dir().ok())
      .unwrap_or_else(|| PathBuf::from("."));
    let existing = self.agent_launch_plan.as_ref().map(|plan| &plan.metadata);
    let mcp_url = self
      .mcp_server
      .as_ref()
      .map(|server| server.url.clone())
      .or_else(|| existing.map(|metadata| metadata.mcp_url.clone()))
      .ok_or_else(|| anyhow::anyhow!("Terminai MCP server is unavailable"))?;
    let mcp_auth_token = self
      .mcp_server
      .as_ref()
      .map(|server| server.auth_token.clone())
      .or_else(|| existing.map(|metadata| metadata.mcp_auth_token.clone()))
      .ok_or_else(|| {
        anyhow::anyhow!("Terminai MCP credentials are unavailable")
      })?;
    let mcp_port = self
      .mcp_server
      .as_ref()
      .map(|server| server.port.to_string())
      .or_else(|| existing.map(|metadata| metadata.terminai_mcp_port.clone()))
      .ok_or_else(|| anyhow::anyhow!("Terminai MCP port is unavailable"))?;
    let binary = existing
      .map(|metadata| metadata.terminai_binary_path.clone())
      .unwrap_or_else(terminai_binary_path);
    let context =
      AgentLaunchContext::new(cwd, mcp_url, mcp_auth_token, binary, mcp_port);
    let mut plan =
      build_launch_plan(agent, &self.config.agent_presets, &context)?;
    normalize_agent_launch_plan_env(&mut plan);
    if !agent_command_available(&plan) {
      anyhow::bail!(
        "Configured AI CLI '{}' was not found in PATH",
        plan.command
      );
    }
    Ok(plan)
  }

  fn switch_agent(&mut self, name: &str, rows: u16, cols: u16) -> Result<()> {
    let config = agent_config_for_choice(&self.config, name);
    let plan = self.build_runtime_agent_plan(&config)?;

    *self.agent_event_rx.lock().unwrap() = None;
    if let Some(mut agent) = self.agent_terminal.take()
      && let Err(err) = agent.terminate()
    {
      log::warn!("Failed to terminate previous AI CLI: {err}");
    }
    self.pending_command = None;
    self.active_agent_name = name.to_string();
    self.active_agent_config = config;
    self.agent_launch_plan = Some(plan);
    self.agent_exit_status = None;
    self.launch_agent(rows, cols);
    Ok(())
  }

  fn handle_control_modal_key(
    &mut self,
    code: KeyCode,
    rows: u16,
    cols: u16,
  ) -> bool {
    let Some(modal) = self.control_modal.clone() else {
      return false;
    };
    match code {
      KeyCode::Esc => self.control_modal = None,
      KeyCode::Up | KeyCode::Left | KeyCode::BackTab => {
        self.control_modal.as_mut().unwrap().previous();
      }
      KeyCode::Down | KeyCode::Right | KeyCode::Tab => {
        self.control_modal.as_mut().unwrap().next();
      }
      KeyCode::Enter => match modal {
        ControlModal::Panel { .. } => match modal.panel_item().unwrap() {
          ControlPanelItem::ApprovalMode => self.request_approval_mode_toggle(),
          ControlPanelItem::Agent => self.open_agent_picker(),
          ControlPanelItem::ClearHistory => {
            self.control_modal = Some(ControlModal::confirm_clear_history())
          }
        },
        ControlModal::AgentPicker { .. } => {
          let name = modal.selected_agent().unwrap_or_default().to_string();
          let config = agent_config_for_choice(&self.config, &name);
          match self.build_runtime_agent_plan(&config) {
            Ok(_) => {
              self.control_modal =
                Some(ControlModal::confirm_agent_switch(name));
            }
            Err(err) => self
              .control_modal
              .as_mut()
              .unwrap()
              .set_error(err.to_string()),
          }
        }
        ControlModal::ConfirmAutoApproval { .. } => {
          self.control_modal = None;
          if modal.is_confirmed() {
            self.approval_mode = ApprovalMode::AutoApproval;
            if self.pending_command.is_some() {
              self.run_approval_action(ApprovalAction::Approve);
            }
          }
        }
        ControlModal::ConfirmClearHistory { .. } => {
          self.control_modal = None;
          if modal.is_confirmed()
            && let Err(err) = self.clear_internal_history()
          {
            self.config_error = Some(format!("Failed to clear history: {err}"));
          }
        }
        ControlModal::ConfirmAgentSwitch { .. } => {
          self.control_modal = None;
          if let Some(name) = modal.confirmed_agent()
            && let Err(err) = self.switch_agent(name, rows, cols)
          {
            self.config_error = Some(format!("Failed to switch agent: {err}"));
          }
        }
      },
      _ => {}
    }
    true
  }

  fn update_shell_cwd(&mut self, cwd: PathBuf) {
    if self.shell_cwd.as_ref() == Some(&cwd) {
      return;
    }

    log::info!("Shell cwd changed to {}", cwd.display());
    self.shell_cwd = Some(cwd.clone());

    if let Some(plan) = self.agent_launch_plan.as_ref() {
      let mut runtime_config = self.config.clone();
      runtime_config.agent = self.active_agent_config.clone();
      match rebuild_agent_launch_plan_for_cwd(
        plan,
        &runtime_config,
        cwd.clone(),
      ) {
        Ok(plan) => self.agent_launch_plan = Some(plan),
        Err(err) => {
          log::error!("Failed to rebuild AI launch plan for cwd change: {err}");
          let mut fallback = plan.clone();
          fallback.cwd = cwd.clone();
          self.agent_launch_plan = Some(fallback);
        }
      }
    }

    if let Some(mcp_state) = &self.mcp_state {
      mcp_state.update_cwd(cwd);
    }
  }

  fn reload_config(&mut self) {
    let env_reload_error =
      if let Err(err) = termin::env_loader::reload_env_file() {
        let message = format!("Failed to reload terminai.env: {err:#}");
        log::error!("{message}");
        Some(message)
      } else {
        None
      };

    let config = match TerminaiConfig::load() {
      Ok(config) => config,
      Err(err) => {
        let message = format!("Failed to reload terminai.yaml: {err:#}");
        log::error!("{message}");
        self.config_error = Some(message);
        return;
      }
    };

    let cwd = self
      .shell_cwd
      .clone()
      .or_else(|| std::env::current_dir().ok())
      .unwrap_or_else(|| PathBuf::from("."));
    let existing_metadata =
      self.agent_launch_plan.as_ref().map(|plan| &plan.metadata);
    let mcp_url = self
      .mcp_server
      .as_ref()
      .map(|server| server.url.clone())
      .or_else(|| existing_metadata.map(|metadata| metadata.mcp_url.clone()));
    let mcp_auth_token = self
      .mcp_server
      .as_ref()
      .map(|server| server.auth_token.clone())
      .or_else(|| {
        existing_metadata.map(|metadata| metadata.mcp_auth_token.clone())
      });
    let terminai_mcp_port = self
      .mcp_server
      .as_ref()
      .map(|server| server.port.to_string())
      .or_else(|| {
        existing_metadata.map(|metadata| metadata.terminai_mcp_port.clone())
      });
    let terminai_binary_path = existing_metadata
      .map(|metadata| metadata.terminai_binary_path.clone())
      .unwrap_or_else(terminai_binary_path);

    let active_was_startup = self.active_agent_config == self.config.agent;
    self.chat_position = config.interface.chat_position;
    self.config = config;
    if active_was_startup {
      self.active_agent_config = self.config.agent.clone();
      self.active_agent_name = startup_agent_name(&self.config);
    }

    if let Some(mcp_state) = &self.mcp_state {
      mcp_state.update_cwd(cwd.clone());
    }

    let (Some(mcp_url), Some(mcp_auth_token), Some(terminai_mcp_port)) =
      (mcp_url, mcp_auth_token, terminai_mcp_port)
    else {
      self.agent_launch_plan = None;
      self.config_error = Some(
        "Reloaded config, but Terminai MCP server is unavailable. Restart to enable AI launches."
          .to_string(),
      );
      return;
    };

    let launch_context = AgentLaunchContext::new(
      cwd,
      mcp_url,
      mcp_auth_token,
      terminai_binary_path,
      terminai_mcp_port,
    );
    let active_plan = build_launch_plan(
      &self.active_agent_config,
      &self.config.agent_presets,
      &launch_context,
    );
    let plan = active_plan.or_else(|err| {
      log::warn!(
        "Active agent '{}' is no longer configured: {err}; falling back to startup agent",
        self.active_agent_name
      );
      self.active_agent_config = self.config.agent.clone();
      self.active_agent_name = startup_agent_name(&self.config);
      build_launch_plan(
        &self.active_agent_config,
        &self.config.agent_presets,
        &launch_context,
      )
    });
    match plan {
      Ok(mut plan) => {
        normalize_agent_launch_plan_env(&mut plan);
        self.agent_launch_plan = Some(plan);
        self.config_error = env_reload_error;
        log::info!("Terminai configuration reloaded");
      }
      Err(err) => {
        let message = format!("Failed to rebuild AI CLI launch plan: {err}");
        log::error!("{message}");
        self.agent_launch_plan = None;
        self.config_error = Some(message);
      }
    }
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

  fn relaunch_agent(&mut self, rows: u16, cols: u16) {
    self.launch_agent(rows, cols);
  }

  fn append_agent_exit_message(&mut self, code: i32) {
    if let Some(agent) = &mut self.agent_terminal
      && let Ok(mut vt) = agent.shell_mut().vt.write()
    {
      let message = format!(
        "\r\n\r\nAI process exited with status {code}.\r\n\r\nPress Enter to relaunch."
      );
      vt.process(message.as_bytes());
      self.agent_view.follow_tail = true;
      self.agent_output_pending = true;
    }
  }

  fn launch_agent(&mut self, rows: u16, cols: u16) {
    let Some(plan) = self.agent_launch_plan.clone() else {
      self.config_error = Some(
        "AI CLI cannot be launched because no launch plan is available."
          .to_string(),
      );
      self.agent_exit_status = None;
      return;
    };

    if !agent_command_available(&plan) {
      self.agent_terminal = None;
      self.agent_exit_status = None;
      self.config_error = Some(format!(
        "Configured AI CLI '{}' was not found in PATH",
        plan.command
      ));
      return;
    }

    match spawn_agent_from_plan(&plan, rows, cols) {
      Ok((agent, rx)) => {
        log::info!("AI CLI terminal started: {}", plan.command);
        self.agent_terminal = Some(agent);
        self.agent_modal_title = modal_title_for_agent(&plan);
        *self.agent_event_rx.lock().unwrap() = Some(rx);
        self.agent_view = TerminalViewState::new();
        self.agent_output_pending = true;
        self.agent_exit_status = None;
        self.config_error = None;
      }
      Err(err) => {
        self.agent_terminal = None;
        self.agent_exit_status = None;
        self.config_error =
          Some(format!("Failed to start AI CLI '{}': {err}", plan.command));
      }
    }
  }

  /// Handle mouse events when AI overlay is visible
  fn handle_ai_mouse_event(
    &mut self,
    mouse: &crossterm::event::MouseEvent,
    terminal_area: Rect,
  ) -> Control<AppEvent> {
    use crossterm::event::MouseEventKind;

    if self.control_modal.is_some() {
      return Control::Changed;
    }

    let overlay_area = self.overlay_area(terminal_area);
    let inner_area = self.overlay_inner_area(terminal_area);

    if self.pending_command.is_some()
      && matches!(
        mouse.kind,
        MouseEventKind::Down(crossterm::event::MouseButton::Left)
      )
      && let Some(action) =
        approval_action_at(terminal_area, mouse.column, mouse.row)
    {
      self.run_approval_action(action);
      return Control::Changed;
    }

    if self.pending_command.is_some()
      && matches!(
        mouse.kind,
        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
      )
      && Self::point_in_rect(
        mouse.column,
        mouse.row,
        approval_modal_area(terminal_area),
      )
    {
      let delta = match mouse.kind {
        MouseEventKind::ScrollUp => -3,
        MouseEventKind::ScrollDown => 3,
        _ => 0,
      };
      self.scroll_approval(delta, terminal_area);
      return Control::Changed;
    }

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

  if let Some(recovery) = &state.recovery {
    recovery.mark_running();
  }
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

  // Push pending VT rows to the host terminal's native scrollback in one
  // backend stream, instead of throttling to one viewport per render.
  let scroll_snapshot = if let Ok(mut vt) = state.shell.vt.write() {
    drain_pending_native_scrollback_snapshot(&mut vt, area.width)
  } else {
    log::error!("Failed to acquire write lock on VT");
    None
  };

  if let Some((content, scroll_up_lines, row_wrapped)) = scroll_snapshot {
    log::trace!(
      "Scrolling up {} lines (pending: {})",
      scroll_up_lines,
      state
        .shell
        .vt
        .read()
        .map(|vt| vt.pending_native_scrollback_len() > 0)
        .unwrap_or(false)
    );
    frame.set_scroll_snapshot(
      content,
      area.width,
      scroll_up_lines,
      row_wrapped,
    );
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
    let title = format!(" {} ", state.agent_modal_title);
    let mut block = Block::default()
      .borders(Borders::ALL)
      .title(title)
      .style(Style::default().fg(Color::Cyan).bg(Color::Black));
    if let Some(status) = approval_status(state.approval_mode) {
      block = block.title(status);
    }
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

        if state.agent_exit_status.is_some() {
          ctx.set_screen_cursor(None);
        } else if !screen.hide_cursor() {
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
    } else if let Some(status) = state.agent_exit_status {
      let message = format!(
        "AI process exited with status {status}.\n\nPress Enter to relaunch."
      );
      Paragraph::new(message)
        .style(Style::default().fg(Color::White))
        .render(inner_area, buf);
      ctx.set_screen_cursor(None);
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

    state.clamp_approval_scroll(area);
    if let Some(pending) = &state.pending_command {
      render_shell_input_approval_with_state(
        area,
        buf,
        pending,
        state.approval_scroll,
        state.approval_focus,
      );
      ctx.set_screen_cursor(None);
    }

    if let Some(modal) = &state.control_modal {
      render_control_modal(
        area,
        buf,
        modal,
        state.approval_mode,
        &state.agent_modal_title,
      );
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

  // Check for VT rows waiting to be pushed into native scrollback.
  if let Ok(vt) = state.shell.vt.read() {
    if vt.pending_native_scrollback_len() > 0 {
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

          // Always close the overlay when deactivate key is pressed
          state.deactivate_ai_overlay()?;
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
        let bindings = &state.config.interface.key_bindings;
        if bindings.control_panel.matches(key_combo) {
          state.control_modal = if state.control_modal.is_some() {
            None
          } else {
            Some(ControlModal::panel())
          };
          break 'm Control::Changed;
        }
        if bindings.toggle_approval_mode.matches(key_combo) {
          state.request_approval_mode_toggle();
          break 'm Control::Changed;
        }
        if bindings.switch_agent.matches(key_combo) {
          state.open_agent_picker();
          break 'm Control::Changed;
        }
        if bindings.clear_history.matches(key_combo) {
          state.control_modal = Some(ControlModal::confirm_clear_history());
          break 'm Control::Changed;
        }
      }

      if state.control_modal.is_some()
        && matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat)
      {
        let (cols, rows) = crossterm::terminal::size()?;
        state.handle_control_modal_key(*code, rows, cols);
        break 'm Control::Changed;
      }

      if state.agent_exit_status.is_some()
        && matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat)
        && matches!(code, KeyCode::Enter)
      {
        let (cols, rows) = crossterm::terminal::size()?;
        state.relaunch_agent(rows, cols);
        break 'm Control::Changed;
      }

      if state.pending_command.is_some()
        && matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat)
        && matches!(
          code,
          KeyCode::Left
            | KeyCode::Right
            | KeyCode::Tab
            | KeyCode::BackTab
            | KeyCode::Enter
            | KeyCode::Up
            | KeyCode::Down
            | KeyCode::PageUp
            | KeyCode::PageDown
            | KeyCode::Home
            | KeyCode::End
        )
      {
        let (cols, rows) = crossterm::terminal::size()?;
        let terminal_area = Rect::new(0, 0, cols, rows);
        let viewport_rows = approval_viewport_height(terminal_area);
        match code {
          KeyCode::Left | KeyCode::Right | KeyCode::Tab | KeyCode::BackTab => {
            state.toggle_approval_focus();
          }
          KeyCode::Enter => {
            state.activate_focused_approval();
          }
          KeyCode::Up => {
            state.scroll_approval(-1, terminal_area);
          }
          KeyCode::Down => {
            state.scroll_approval(1, terminal_area);
          }
          KeyCode::PageUp => {
            state.scroll_approval(
              -(viewport_rows.saturating_sub(1).max(1) as isize),
              terminal_area,
            );
          }
          KeyCode::PageDown => {
            state.scroll_approval(
              viewport_rows.saturating_sub(1).max(1) as isize,
              terminal_area,
            );
          }
          KeyCode::Home => {
            state.scroll_approval(
              -(state.approval_scroll as isize),
              terminal_area,
            );
          }
          KeyCode::End => {
            state.approval_scroll = state.max_approval_scroll(terminal_area);
          }
          _ => {}
        }
        break 'm Control::Changed;
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
      } else if state.control_modal.is_none()
        && let Some(agent) = &mut state.agent_terminal
      {
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
    AppEvent::ShellHostEscape(escape) => {
      let mut stdout = std::io::stdout();
      stdout.write_all(escape.as_bytes())?;
      stdout.flush()?;
      if let Some(cwd) = cwd_from_osc7_escape(escape) {
        state.update_shell_cwd(cwd);
      }
      log::trace!("Shell host escape forwarded");
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
      state.append_agent_exit_message(*code);
      state.agent_exit_status = Some(*code);
      Control::Changed
    }
    AppEvent::ConfigChanged => {
      log::info!("Config file change detected");
      state.reload_config();
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
  Err(error)
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
  fn auto_approval_does_not_treat_risk_classification_as_a_boundary() {
    for risk in [
      termin::command::RiskLevel::Safe,
      termin::command::RiskLevel::Caution,
      termin::command::RiskLevel::Dangerous,
    ] {
      assert_eq!(
        suggestion_action(ApprovalMode::AlwaysAsk, risk),
        SuggestionAction::Ask
      );
      assert_eq!(
        suggestion_action(ApprovalMode::AutoApproval, risk),
        SuggestionAction::AutoApprove
      );
    }
  }

  #[test]
  fn auto_approval_status_is_explicit_and_warn_colored() {
    assert!(approval_status(ApprovalMode::AlwaysAsk).is_none());

    let status = approval_status(ApprovalMode::AutoApproval).unwrap();
    assert_eq!(status.spans[0].content, " ⚠ AUTO-APPROVE ");
    assert_eq!(status.spans[0].style.fg, Some(Color::Yellow));
  }

  #[test]
  fn switcher_includes_builtins_visible_presets_and_distinct_startup_agent() {
    let mut config = TerminaiConfig::default();
    config.agent = termin::terminai_config::AgentConfig {
      preset: None,
      kind: Some(termin::terminai_config::AgentKind::Custom),
      command: Some("private-agent".into()),
      args: Vec::new(),
      extra_args: Vec::new(),
      prompt_template: None,
      uses_mcp: None,
      uses_tool_cli: None,
    };
    config.agent_presets.insert(
      "visible".into(),
      termin::terminai_config::AgentPresetConfig {
        command: Some("visible-agent".into()),
        ..Default::default()
      },
    );
    config.agent_presets.insert(
      "hidden".into(),
      termin::terminai_config::AgentPresetConfig {
        command: Some("hidden-agent".into()),
        show_in_switcher: false,
        ..Default::default()
      },
    );

    let agents = switcher_agents(&config).unwrap();

    assert_eq!(
      agents,
      ["claude", "codex", "opencode", "private-agent", "visible"]
    );
    assert_eq!(
      agent_config_for_choice(&config, "private-agent"),
      config.agent
    );
    assert_eq!(
      agent_config_for_choice(&config, "claude").preset.as_deref(),
      Some("claude")
    );
  }

  #[test]
  fn startup_terminal_size_uses_default_when_any_dimension_is_zero() {
    assert_eq!(startup_terminal_size(0, 0), (80, 24));
    assert_eq!(startup_terminal_size(0, 24), (80, 24));
    assert_eq!(startup_terminal_size(80, 0), (80, 24));
    assert_eq!(startup_terminal_size(120, 40), (120, 40));
  }

  #[test]
  fn agent_pty_size_matches_inner_overlay_area() {
    assert_eq!(agent_pty_size(40, 120), (18, 118));
    assert_eq!(agent_pty_size(8, 1), (8, 1));
  }

  #[test]
  fn cwd_from_osc7_escape_decodes_file_uri() {
    assert_eq!(
      cwd_from_osc7_escape("\x1b]7;file://host/tmp/project%20one\x07"),
      Some(PathBuf::from("/tmp/project one"))
    );
    assert_eq!(
      cwd_from_osc7_escape("\x1b]7;file:///tmp/project\x1b\\"),
      Some(PathBuf::from("/tmp/project"))
    );
    assert_eq!(cwd_from_osc7_escape("\x1b]2;title\x07"), None);
    assert_eq!(
      cwd_from_osc7_escape("\x1b]7;file:///C:\\Users\\A%20B\x07"),
      Some(PathBuf::from("C:/Users/A B"))
    );
  }

  #[test]
  fn cli_parses_init_config_subcommand() {
    let args =
      Args::try_parse_from(["terminai", "init-config", "--force"]).unwrap();

    match args.subcommand {
      Some(CliCommand::InitConfig { force }) => assert!(force),
      _ => panic!("expected init-config subcommand"),
    }
    assert!(args.command.is_empty());
  }

  #[test]
  fn cli_version_uses_binary_name_and_package_version() {
    let mut command = <Args as clap::CommandFactory>::command();

    assert_eq!(
      command.render_version().to_string(),
      format!("terminai {}\n", env!("CARGO_PKG_VERSION"))
    );
  }

  #[test]
  fn cli_parses_hidden_mcp_subcommand() {
    let args = Args::try_parse_from(["terminai", "_mcp"]).unwrap();

    match args.subcommand {
      Some(CliCommand::Mcp) => {}
      _ => panic!("expected _mcp subcommand"),
    }
    assert!(args.command.is_empty());
    assert!(Args::try_parse_from(["terminai", "_mcp", "4321"]).is_err());
  }

  #[test]
  fn cli_parses_hidden_tool_subcommands() {
    let args = Args::try_parse_from([
      "terminai",
      "tool",
      "--json",
      "read_terminal",
      "--max-lines",
      "40",
      "--include-visible",
      "false",
    ])
    .unwrap();

    match args.subcommand {
      Some(CliCommand::Tool {
        json: true,
        command:
          ToolCommand::ReadTerminal {
            max_lines: Some(40),
            include_visible: Some(false),
          },
      }) => {}
      other => panic!("unexpected tool command: {other:?}"),
    }

    let args = Args::try_parse_from([
      "terminai",
      "tool",
      "--json",
      "read-terminal",
      "--max-lines",
      "40",
      "--include-visible",
      "false",
    ])
    .unwrap();

    match args.subcommand {
      Some(CliCommand::Tool {
        json: true,
        command:
          ToolCommand::ReadTerminal {
            max_lines: Some(40),
            include_visible: Some(false),
          },
      }) => {}
      other => panic!("unexpected tool command: {other:?}"),
    }

    for command in [
      "check-for-updates",
      "get-terminal-context",
      "get-suggestion-status",
    ] {
      assert!(
        Args::try_parse_from(["terminai", "tool", command]).is_ok(),
        "expected {command} alias to parse"
      );
    }

    let args = Args::try_parse_from([
      "terminai",
      "tool",
      "suggest_input",
      "git status\r",
      "--explanation",
      "Check repository status.",
    ])
    .unwrap();

    match args.subcommand {
      Some(CliCommand::Tool {
        json: false,
        command: ToolCommand::SuggestInput { input, explanation },
      }) => {
        assert_eq!(input, "git status\r");
        assert_eq!(explanation.as_deref(), Some("Check repository status."));
      }
      other => panic!("unexpected tool command: {other:?}"),
    }

    let args = Args::try_parse_from([
      "terminai",
      "tool",
      "suggest-input",
      "git status\r",
      "--explanation",
      "Check repository status.",
    ])
    .unwrap();

    match args.subcommand {
      Some(CliCommand::Tool {
        json: false,
        command: ToolCommand::SuggestInput { input, explanation },
      }) => {
        assert_eq!(input, "git status\r");
        assert_eq!(explanation.as_deref(), Some("Check repository status."));
      }
      other => panic!("unexpected tool command: {other:?}"),
    }
  }

  #[test]
  fn hidden_tool_commands_map_to_mcp_requests() {
    let request = tool_request(ToolCommand::ReadTerminal {
      max_lines: Some(7),
      include_visible: Some(true),
    })
    .unwrap();
    assert_eq!(request.name, READ_TERMINAL);
    let arguments = request.arguments.unwrap();
    assert_eq!(arguments["max_lines"], serde_json::json!(7));
    assert_eq!(arguments["include_visible"], serde_json::json!(true));

    let request = tool_request(ToolCommand::ReadTerminal {
      max_lines: None,
      include_visible: None,
    })
    .unwrap();
    assert_eq!(request.name, READ_TERMINAL);
    assert!(request.arguments.unwrap().is_empty());

    let request = tool_request(ToolCommand::CheckForUpdates).unwrap();
    assert_eq!(request.name, CHECK_FOR_UPDATES);
    assert!(request.arguments.is_none());

    let request = tool_request(ToolCommand::GetTerminalContext).unwrap();
    assert_eq!(request.name, GET_TERMINAL_CONTEXT);
    assert!(request.arguments.is_none());

    let request = tool_request(ToolCommand::SuggestInput {
      input: "make test\r".to_string(),
      explanation: None,
    })
    .unwrap();
    assert_eq!(request.name, SUGGEST_INPUT);
    let arguments = request.arguments.unwrap();
    assert_eq!(arguments["input"], serde_json::json!("make test\r"));
    assert!(!arguments.contains_key("explanation"));

    let request = tool_request(ToolCommand::GetSuggestionStatus).unwrap();
    assert_eq!(request.name, GET_SUGGESTION_STATUS);
    assert!(request.arguments.is_none());
  }

  #[test]
  fn cli_preserves_trailing_shell_command() {
    let args =
      Args::try_parse_from(["terminai", "--", "echo", "hello"]).unwrap();

    assert!(args.subcommand.is_none());
    assert_eq!(args.command, vec!["echo", "hello"]);
  }

  #[test]
  fn cli_help_lists_init_config() {
    let mut command = <Args as clap::CommandFactory>::command();
    let mut help = Vec::new();
    command.write_long_help(&mut help).unwrap();
    let help = String::from_utf8(help).unwrap();

    assert!(help.contains("init-config"));
    assert!(!help.contains("_mcp"));
    assert!(!help.contains("tool"));
  }

  #[test]
  fn recovery_uses_requested_command_before_startup() {
    assert_eq!(
      recovery_fallback(RecoveryPhase::Starting),
      RecoveryFallback::RequestedCommand
    );
  }

  #[test]
  fn recovery_uses_interactive_shell_after_startup() {
    assert_eq!(
      recovery_fallback(RecoveryPhase::Running),
      RecoveryFallback::InteractiveShell
    );
  }

  #[test]
  fn rebuild_agent_launch_plan_updates_expanded_cwd_args() {
    let config = TerminaiConfig {
      agent: termin::terminai_config::AgentConfig {
        command: Some("my-agent".to_string()),
        args: vec!["--workdir".into(), "{{cwd}}".into()],
        ..Default::default()
      },
      ..Default::default()
    };
    let old_context = AgentLaunchContext::new(
      PathBuf::from("/old/project"),
      "http://127.0.0.1:1234/mcp".to_string(),
      "old-token".to_string(),
      "/usr/bin/terminai".to_string(),
      "1234".to_string(),
    );
    let old_plan =
      build_launch_plan(&config.agent, &config.agent_presets, &old_context)
        .unwrap();

    let new_plan = rebuild_agent_launch_plan_for_cwd(
      &old_plan,
      &config,
      PathBuf::from("/new/project"),
    )
    .unwrap();

    assert_eq!(new_plan.cwd, PathBuf::from("/new/project"));
    assert!(new_plan.args.windows(2).any(|window| {
      window[0] == "--workdir" && window[1] == "/new/project"
    }));
    assert!(!new_plan.args.iter().any(|arg| arg == "/old/project"));
    assert_eq!(
      new_plan
        .env
        .get("TERMINAI_MCP_AUTH_TOKEN")
        .map(String::as_str),
      Some("old-token")
    );
    assert_eq!(
      new_plan.env.get("TERMINAI_MCP_PORT").map(String::as_str),
      Some("1234")
    );
    assert!(!new_plan.env.contains_key("TERMINAI_MCP_URL"));
    assert!(!new_plan.env.contains_key("TERMINAI_MCP_COMMAND"));
    assert_eq!(new_plan.metadata.mcp_url, "http://127.0.0.1:1234/mcp");
    assert_eq!(new_plan.metadata.mcp_auth_token, "old-token");
    assert_eq!(new_plan.metadata.terminai_binary_path, "/usr/bin/terminai");
    assert_eq!(
      new_plan.metadata.terminai_mcp_command,
      "/usr/bin/terminai _mcp"
    );
    assert_eq!(new_plan.metadata.terminai_mcp_port, "1234");
  }

  #[test]
  fn augmented_agent_path_preserves_existing_path_and_adds_user_bins() {
    let path = augmented_agent_path(Some(&"/custom/bin".to_string())).unwrap();
    let paths: Vec<PathBuf> =
      std::env::split_paths(std::ffi::OsStr::new(&path)).collect();

    assert_eq!(paths.first(), Some(&PathBuf::from("/custom/bin")));
    assert!(paths.contains(&PathBuf::from("/usr/local/bin")));
    assert!(paths.contains(&PathBuf::from("/opt/homebrew/bin")));
  }

  #[test]
  fn normalize_agent_launch_plan_sets_augmented_path() {
    let mut plan = AgentLaunchPlan {
      command: "codex".to_string(),
      args: Vec::new(),
      env: std::collections::HashMap::new(),
      cwd: PathBuf::from("/tmp"),
      metadata: Default::default(),
    };

    normalize_agent_launch_plan_env(&mut plan);

    let path = plan.env.get("PATH").expect("PATH should be set");
    let paths: Vec<PathBuf> =
      std::env::split_paths(std::ffi::OsStr::new(path)).collect();
    assert!(paths.contains(&PathBuf::from("/usr/local/bin")));
  }

  #[test]
  fn modal_title_uses_launched_agent_command_name() {
    let plan = AgentLaunchPlan {
      command: "/usr/local/bin/custom-agent".to_string(),
      args: Vec::new(),
      env: std::collections::HashMap::new(),
      cwd: PathBuf::from("/tmp"),
      metadata: Default::default(),
    };

    assert_eq!(modal_title_for_agent(&plan), "custom-agent");
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

  #[test]
  #[cfg(not(windows))]
  fn deactivate_overlay_keeps_pending_command_for_reopen() {
    let (suggestion_tx, suggestion_rx) = mpsc::unbounded_channel();
    drop(suggestion_tx);

    let (shell, _shell_rx) = Shell::spawn_command(
      "sh",
      &["-c".to_string(), "true".to_string()],
      24,
      80,
    )
    .unwrap();
    let pending = PendingCommand::new(
      "git status".to_string(),
      Some("Inspect worktree".to_string()),
      termin::command::RiskLevel::Safe,
    );

    let mut state = AppState {
      shell_cwd: None,
      mcp_state: None,
      shell,
      agent_terminal: None,
      mcp_server: None,
      agent_launch_plan: None,
      active_agent_name: "codex".into(),
      active_agent_config: AgentConfig::codex(),
      agent_event_rx: Arc::new(std::sync::Mutex::new(None)),
      agent_view: TerminalViewState::new(),
      suggestion_rx,
      pending_command: Some(pending),
      approval_mode: ApprovalMode::AlwaysAsk,
      approval_scroll: 0,
      approval_focus: ApprovalAction::Approve,
      control_modal: None,
      agent_exit_status: None,
      agent_modal_title: "AI Terminal".to_string(),
      ai_visible: true,
      chat_position: ChatPosition::Bottom,
      scrollback_tracker: ScrollbackTracker::new(),
      config: TerminaiConfig::default(),
      config_error: None,
      key_combiner: Combiner::default(),
      shell_output_pending: false,
      agent_output_pending: false,
      recovery: None,
    };

    state.deactivate_ai_overlay().unwrap();

    assert!(!state.ai_visible);
    assert!(state.pending_command.is_some());
  }

  #[test]
  fn approval_scroll_clamps_to_pending_content() {
    let (suggestion_tx, suggestion_rx) = mpsc::unbounded_channel();
    drop(suggestion_tx);

    let (shell, _shell_rx) = Shell::spawn_command(
      "sh",
      &["-c".to_string(), "true".to_string()],
      24,
      80,
    )
    .unwrap();
    let pending = PendingCommand::new(
      [
        "echo line0",
        "line1",
        "line2",
        "line3",
        "line4",
        "line5",
        "line6",
        "line7",
        "line8",
      ]
      .join("\\n"),
      Some("Long approval content should be scrollable.".to_string()),
      termin::command::RiskLevel::Safe,
    );

    let mut state = AppState {
      shell_cwd: None,
      mcp_state: None,
      shell,
      agent_terminal: None,
      mcp_server: None,
      agent_launch_plan: None,
      active_agent_name: "codex".into(),
      active_agent_config: AgentConfig::codex(),
      agent_event_rx: Arc::new(std::sync::Mutex::new(None)),
      agent_view: TerminalViewState::new(),
      suggestion_rx,
      pending_command: Some(pending),
      approval_mode: ApprovalMode::AlwaysAsk,
      approval_scroll: 0,
      approval_focus: ApprovalAction::Approve,
      control_modal: None,
      agent_exit_status: None,
      agent_modal_title: "AI Terminal".to_string(),
      ai_visible: true,
      chat_position: ChatPosition::Bottom,
      scrollback_tracker: ScrollbackTracker::new(),
      config: TerminaiConfig::default(),
      config_error: None,
      key_combiner: Combiner::default(),
      shell_output_pending: false,
      agent_output_pending: false,
      recovery: None,
    };

    state.scroll_approval(100, Rect::new(0, 0, 80, 24));
    assert_eq!(state.approval_scroll, 6);

    state.scroll_approval(-2, Rect::new(0, 0, 80, 24));
    assert_eq!(state.approval_scroll, 4);
  }

  #[test]
  fn approval_focus_toggles_and_return_activates_focused_button() {
    let (suggestion_tx, suggestion_rx) = mpsc::unbounded_channel();
    drop(suggestion_tx);

    let (shell, _shell_rx) = Shell::spawn_command(
      "sh",
      &["-c".to_string(), "true".to_string()],
      24,
      80,
    )
    .unwrap();
    let pending = PendingCommand::new(
      "git status".to_string(),
      Some("Inspect worktree".to_string()),
      termin::command::RiskLevel::Safe,
    );

    let mut state = AppState {
      shell_cwd: None,
      mcp_state: None,
      shell,
      agent_terminal: None,
      mcp_server: None,
      agent_launch_plan: None,
      active_agent_name: "codex".into(),
      active_agent_config: AgentConfig::codex(),
      agent_event_rx: Arc::new(std::sync::Mutex::new(None)),
      agent_view: TerminalViewState::new(),
      suggestion_rx,
      pending_command: Some(pending),
      approval_mode: ApprovalMode::AlwaysAsk,
      approval_scroll: 0,
      approval_focus: ApprovalAction::Approve,
      control_modal: None,
      agent_exit_status: None,
      agent_modal_title: "AI Terminal".to_string(),
      ai_visible: true,
      chat_position: ChatPosition::Bottom,
      scrollback_tracker: ScrollbackTracker::new(),
      config: TerminaiConfig::default(),
      config_error: None,
      key_combiner: Combiner::default(),
      shell_output_pending: false,
      agent_output_pending: false,
      recovery: None,
    };

    state.toggle_approval_focus();
    assert_eq!(state.approval_focus, ApprovalAction::Deny);

    state.activate_focused_approval();
    assert!(state.pending_command.is_none());
  }

  #[test]
  fn approving_suggestion_closes_ai_modal() {
    let (suggestion_tx, suggestion_rx) = mpsc::unbounded_channel();
    drop(suggestion_tx);

    let (shell, _shell_rx) = Shell::spawn_command(
      "sh",
      &["-c".to_string(), "true".to_string()],
      24,
      80,
    )
    .unwrap();
    let pending = PendingCommand::new(
      "git status".to_string(),
      Some("Inspect worktree".to_string()),
      termin::command::RiskLevel::Safe,
    );

    let mut state = AppState {
      shell_cwd: None,
      mcp_state: None,
      shell,
      agent_terminal: None,
      mcp_server: None,
      agent_launch_plan: None,
      active_agent_name: "codex".into(),
      active_agent_config: AgentConfig::codex(),
      agent_event_rx: Arc::new(std::sync::Mutex::new(None)),
      agent_view: TerminalViewState::new(),
      suggestion_rx,
      pending_command: Some(pending),
      approval_mode: ApprovalMode::AlwaysAsk,
      approval_scroll: 0,
      approval_focus: ApprovalAction::Approve,
      control_modal: None,
      agent_exit_status: None,
      agent_modal_title: "AI Terminal".to_string(),
      ai_visible: true,
      chat_position: ChatPosition::Bottom,
      scrollback_tracker: ScrollbackTracker::new(),
      config: TerminaiConfig::default(),
      config_error: None,
      key_combiner: Combiner::default(),
      shell_output_pending: false,
      agent_output_pending: false,
      recovery: None,
    };

    state.run_approval_action(ApprovalAction::Approve);

    assert!(state.pending_command.is_none());
    assert!(!state.ai_visible);
  }

  #[test]
  fn approval_button_click_activates_action() {
    let (suggestion_tx, suggestion_rx) = mpsc::unbounded_channel();
    drop(suggestion_tx);

    let (shell, _shell_rx) = Shell::spawn_command(
      "sh",
      &["-c".to_string(), "true".to_string()],
      24,
      80,
    )
    .unwrap();
    let pending = PendingCommand::new(
      "git status".to_string(),
      Some("Inspect worktree".to_string()),
      termin::command::RiskLevel::Safe,
    );

    let mut state = AppState {
      shell_cwd: None,
      mcp_state: None,
      shell,
      agent_terminal: None,
      mcp_server: None,
      agent_launch_plan: None,
      active_agent_name: "codex".into(),
      active_agent_config: AgentConfig::codex(),
      agent_event_rx: Arc::new(std::sync::Mutex::new(None)),
      agent_view: TerminalViewState::new(),
      suggestion_rx,
      pending_command: Some(pending),
      approval_mode: ApprovalMode::AlwaysAsk,
      approval_scroll: 0,
      approval_focus: ApprovalAction::Approve,
      control_modal: None,
      agent_exit_status: None,
      agent_modal_title: "AI Terminal".to_string(),
      ai_visible: true,
      chat_position: ChatPosition::Bottom,
      scrollback_tracker: ScrollbackTracker::new(),
      config: TerminaiConfig::default(),
      config_error: None,
      key_combiner: Combiner::default(),
      shell_output_pending: false,
      agent_output_pending: false,
      recovery: None,
    };

    let terminal_area = Rect::new(0, 0, 80, 24);
    let deny = termin::ui_approval::approval_button_areas(terminal_area).deny;
    let mouse = crossterm::event::MouseEvent {
      kind: crossterm::event::MouseEventKind::Down(
        crossterm::event::MouseButton::Left,
      ),
      column: deny.x,
      row: deny.y,
      modifiers: crossterm::event::KeyModifiers::NONE,
    };

    let result = state.handle_ai_mouse_event(&mouse, terminal_area);

    assert!(matches!(result, Control::Changed));
    assert!(state.pending_command.is_none());
  }

  #[test]
  fn agent_output_events_are_coalesced_without_dropping_control_events() {
    let mut events = VecDeque::new();
    let output_wakeup = OutputWakeup::new();

    push_coalesced_output_event(
      &mut events,
      ShellEvent::Output(output_wakeup.clone()),
    );
    push_coalesced_output_event(
      &mut events,
      ShellEvent::Output(output_wakeup.clone()),
    );
    push_coalesced_output_event(
      &mut events,
      ShellEvent::TermReply("reply".into()),
    );
    push_coalesced_output_event(
      &mut events,
      ShellEvent::Output(output_wakeup.clone()),
    );
    push_coalesced_output_event(
      &mut events,
      ShellEvent::Output(output_wakeup.clone()),
    );
    push_coalesced_output_event(&mut events, ShellEvent::Exited(0));

    assert!(matches!(events.pop_front(), Some(ShellEvent::Output(_))));
    assert!(matches!(
      events.pop_front(),
      Some(ShellEvent::TermReply(reply)) if reply == "reply"
    ));
    assert!(matches!(events.pop_front(), Some(ShellEvent::Output(_))));
    assert!(matches!(events.pop_front(), Some(ShellEvent::Exited(0))));
    assert!(events.is_empty());
  }

  #[test]
  fn shell_output_events_are_coalesced_without_dropping_control_events() {
    let mut events = VecDeque::new();
    let output_wakeup = OutputWakeup::new();

    push_coalesced_shell_event(
      &mut events,
      ShellEvent::Output(output_wakeup.clone()),
    );
    push_coalesced_shell_event(
      &mut events,
      ShellEvent::Output(output_wakeup.clone()),
    );
    push_coalesced_shell_event(
      &mut events,
      ShellEvent::HostEscape("escape".into()),
    );
    push_coalesced_shell_event(
      &mut events,
      ShellEvent::Output(output_wakeup.clone()),
    );
    push_coalesced_shell_event(
      &mut events,
      ShellEvent::Output(output_wakeup.clone()),
    );
    push_coalesced_shell_event(&mut events, ShellEvent::Exited(0));

    assert!(matches!(events.pop_front(), Some(ShellEvent::Output(_))));
    assert!(matches!(
      events.pop_front(),
      Some(ShellEvent::HostEscape(escape)) if escape == "escape"
    ));
    assert!(matches!(events.pop_front(), Some(ShellEvent::Output(_))));
    assert!(matches!(events.pop_front(), Some(ShellEvent::Exited(0))));
    assert!(events.is_empty());
  }
}
