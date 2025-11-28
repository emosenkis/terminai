// Termin.AI - Single-shell terminal with AI overlay using Hybrid Terminal System
// Integrates hybrid terminal components for proper mode management and output buffering

use anyhow::Result;
use crossterm::event::{
  self, Event as CrosstermEvent, KeyCode, KeyEvent, KeyModifiers,
};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::io::Write;
use std::sync::{Arc, Mutex as StdMutex, RwLock as StdRwLock};
use std::time::Duration;
use tokio::sync::{Mutex, RwLock, mpsc};

// Import hybrid terminal system components
use termin::hybrid::{
  input::key_to_bytes,
  mode::{Mode, ModeManager},
  rendering::{ModalState, RatatuiRenderer},
  routing::{OutputBuffer, OutputRouter},
  terminal::{HostTerminalController, ShadowTerminal},
};

// Import AI and other components
use termin::ai_proc::AIChatProcess;
use termin::llm::{Provider, TerminalContext};
use termin::vt100::TermReplySender;

// ============================================================================
// PTY Bridge - Manages PTY and channels
// ============================================================================

struct PtyBridge {
  master: Box<dyn portable_pty::MasterPty + Send>,
  exit_code: Arc<StdRwLock<Option<u32>>>,
}

impl PtyBridge {
  /// Spawn a shell PTY and return bridge + channels
  fn spawn(
    shell_cmd: &str,
    rows: u16,
    cols: u16,
  ) -> Result<(
    Self,
    mpsc::UnboundedReceiver<Vec<u8>>, // PTY output
    mpsc::UnboundedSender<Vec<u8>>,   // PTY input
  )> {
    log::info!("Spawning shell: {} ({}x{})", shell_cmd, cols, rows);

    // Create channels
    let (output_tx, output_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (input_tx, mut input_rx) = mpsc::unbounded_channel::<Vec<u8>>();

    // Create PTY
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
      rows,
      cols,
      pixel_width: 0,
      pixel_height: 0,
    })?;

    // Build and spawn command
    let mut cmd = CommandBuilder::new(shell_cmd);
    cmd.env("TERM", "xterm-256color");
    cmd.env("TERMINAI", "1");

    // Spawn command
    let mut child = pair.slave.spawn_command(cmd)?;
    let pid = child.process_id().unwrap_or(0);
    log::info!("Shell spawned with PID: {}", pid);

    // Get reader and writer for PTY
    let mut reader = pair.master.try_clone_reader()?;
    let mut writer = pair.master.take_writer()?;

    // Spawn thread to read PTY output and send to channel
    std::thread::spawn(move || {
      let mut buf = vec![0u8; 32 * 1024];
      loop {
        match reader.read(&mut buf) {
          Ok(0) => break, // EOF
          Ok(n) => {
            if output_tx.send(buf[..n].to_vec()).is_err() {
              break;
            }
          }
          Err(e) => {
            log::error!("PTY read error: {}", e);
            break;
          }
        }
      }
      log::info!("PTY reader thread exiting");
    });

    // Spawn task to write input from channel to PTY
    tokio::spawn(async move {
      while let Some(data) = input_rx.recv().await {
        if writer.write_all(&data).is_err() {
          break;
        }
        if writer.flush().is_err() {
          break;
        }
      }
      log::info!("PTY writer task exiting");
    });

    // Spawn thread to wait for child exit
    let exit_code = Arc::new(StdRwLock::new(None));
    let exit_code_clone = exit_code.clone();
    std::thread::spawn(move || {
      let code = match child.wait() {
        Ok(status) => status.exit_code(),
        Err(_) => 1,
      };
      log::info!("Shell exited with code: {}", code);
      *exit_code_clone.write().unwrap() = Some(code);
    });

    Ok((
      Self {
        master: pair.master,
        exit_code,
      },
      output_rx,
      input_tx,
    ))
  }

  fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
    self.master.resize(PtySize {
      rows,
      cols,
      pixel_width: 0,
      pixel_height: 0,
    })?;
    log::info!("PTY resized to {}x{}", cols, rows);
    Ok(())
  }

  fn check_exit(&self) -> Option<u32> {
    *self.exit_code.read().unwrap()
  }
}

// ============================================================================
// Terminal Reply Sender - Implementation for vt100 terminal queries
// ============================================================================

#[derive(Clone)]
struct TerminalReplySender {
  pty_input_tx: mpsc::UnboundedSender<Vec<u8>>,
}

impl TermReplySender for TerminalReplySender {
  fn reply(&self, s: compact_str::CompactString) {
    // Send terminal query replies back to PTY
    let _ = self.pty_input_tx.send(s.as_bytes().to_vec());
  }
}

// ============================================================================
// Main App - Coordinates hybrid terminal components and AI
// ============================================================================

struct App {
  // Hybrid terminal components (used directly for better control)
  mode_manager: Arc<RwLock<ModeManager>>,
  shadow_terminal: Arc<RwLock<ShadowTerminal<TerminalReplySender>>>,
  host_controller: Arc<Mutex<HostTerminalController>>,
  output_router: OutputRouter<TerminalReplySender>,
  renderer: Arc<StdMutex<Option<RatatuiRenderer>>>,
  output_buffer: Arc<Mutex<OutputBuffer>>,

  // PTY bridge and channels
  pty_bridge: PtyBridge,
  pty_output_rx: mpsc::UnboundedReceiver<Vec<u8>>,
  pty_input_tx: mpsc::UnboundedSender<Vec<u8>>,

  // AI assistant
  ai_process: Option<AIChatProcess>,
  modal_state: Arc<StdMutex<Option<ModalState>>>,
}

impl App {
  async fn new(shell_cmd: String) -> Result<Self> {
    // Get terminal size
    let (cols, rows) = crossterm::terminal::size()?;

    // Create PTY bridge
    let (pty_bridge, pty_output_rx, pty_input_tx) =
      PtyBridge::spawn(&shell_cmd, rows, cols)?;

    // Create reply sender for vt100 terminal queries
    let reply_sender = TerminalReplySender {
      pty_input_tx: pty_input_tx.clone(),
    };

    // Create shared hybrid terminal components
    let mode_manager = Arc::new(RwLock::new(ModeManager::new()));
    let shadow_terminal = Arc::new(RwLock::new(ShadowTerminal::new(
      cols,
      rows,
      1000, // scrollback lines
      reply_sender,
    )));
    let host_controller = Arc::new(Mutex::new(HostTerminalController::new()));
    let output_buffer = Arc::new(Mutex::new(OutputBuffer::default()));

    let output_router = OutputRouter::new(
      Arc::clone(&mode_manager),
      Arc::clone(&shadow_terminal),
      Arc::clone(&host_controller),
      Arc::clone(&output_buffer),
    );

    // Initialize AI if API key available
    let ai_process = Self::init_ai().await;

    Ok(Self {
      mode_manager,
      shadow_terminal,
      host_controller,
      output_router,
      renderer: Arc::new(StdMutex::new(None)),
      output_buffer,
      pty_bridge,
      pty_output_rx,
      pty_input_tx,
      ai_process,
      modal_state: Arc::new(StdMutex::new(None)),
    })
  }

  /// Initialize AI assistant if API keys are available
  async fn init_ai() -> Option<AIChatProcess> {
    // Try multiple providers in order of preference
    let providers = [
      (Provider::Anthropic, "ANTHROPIC_API_KEY"),
      (Provider::OpenAI, "OPENAI_API_KEY"),
      (Provider::Gemini, "GOOGLE_API_KEY"),
      (Provider::Gemini, "GEMINI_API_KEY"),
      (Provider::OpenRouter, "OPENROUTER_API_KEY"),
    ];

    for (provider, env_key) in &providers {
      if std::env::var(env_key).is_ok() {
        log::info!("Initializing AI with provider: {}", provider);
        match AIChatProcess::new(*provider, None).await {
          Ok(ai_process) => {
            log::info!("AI assistant initialized successfully");
            return Some(ai_process);
          }
          Err(e) => {
            log::warn!("Failed to initialize {} : {:?}", provider, e);
          }
        }
      }
    }

    log::info!("No API keys found - AI features disabled");
    None
  }

  /// Extract terminal context for AI from shadow terminal
  async fn extract_context(&self) -> TerminalContext {
    let mut history_lines = Vec::new();
    let max_lines = 500;

    // Extract from shadow terminal
    {
      let shadow = self.shadow_terminal.read().await;
      let content = shadow.visible_content();

      for row in content.cells.iter().take(max_lines) {
        let mut line = String::new();
        for cell in row {
          line.push_str(&cell.symbol().to_string());
        }
        let trimmed = line.trim_end();
        if !trimmed.is_empty() {
          history_lines.push(trimmed.to_string());
        }
      }
    }

    let cwd =
      std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));
    TerminalContext::new(history_lines, cwd, None)
  }

  /// Update AI modal state for rendering
  fn update_ai_modal(&mut self) {
    if let Some(ref ai) = self.ai_process {
      let mut content = String::new();

      // Show conversation history
      for msg in ai.conversation() {
        match msg.role {
          termin::ai_proc::MessageRole::User => {
            content.push_str(&format!("You: {}\n\n", msg.content));
          }
          termin::ai_proc::MessageRole::Assistant => {
            content.push_str(&format!("AI: {}\n\n", msg.content));
          }
          termin::ai_proc::MessageRole::System => {
            // Skip system messages
          }
        }
      }

      // Show input buffer
      let input = ai.input_buffer();
      if !input.is_empty() || content.is_empty() {
        content.push_str(&format!("\n> {}_", input));
      }

      // Show pending command approval if any
      if let Some(pending) = ai.pending_command() {
        content.push_str(&format!(
          "\n\n─────────────────────────\n\
                     Proposed Command:\n\
                     $ {}\n\
                     ─────────────────────────\n\
                     Approve? (y/n): ",
          pending.command
        ));
      }

      // If no content, show welcome message
      if content.is_empty() {
        content = "AI Assistant\n\nType your message and press Enter to chat.\nPress ESC or Ctrl+Space to close.".to_string();
      }

      let modal =
        ModalState::text("AI Assistant (Ctrl+Space to toggle)", content);
      *self.modal_state.lock().unwrap() = Some(modal);
    } else {
      let modal = ModalState::text(
        "AI Assistant",
        "AI not configured.\n\nSet ANTHROPIC_API_KEY, OPENAI_API_KEY, or GEMINI_API_KEY environment variable.\n\nPress ESC or Ctrl+Space to close.",
      );
      *self.modal_state.lock().unwrap() = Some(modal);
    }
  }

  /// Handle keyboard input
  async fn handle_key_input(&mut self, key: KeyEvent) -> Result<bool> {
    let mode = self.mode_manager.read().await.current_mode();
    let is_modal_visible = self.mode_manager.read().await.is_modal_visible();

    // Check for global shortcuts first
    if key.code == KeyCode::Char(' ')
      && key.modifiers.contains(KeyModifiers::CONTROL)
    {
      self.toggle_modal().await?;
      return Ok(true);
    }

    if key.code == KeyCode::Esc && is_modal_visible {
      self.close_modal().await?;
      return Ok(true);
    }

    // Route based on mode
    match mode {
      Mode::Passthrough | Mode::GuestAltBuffer => {
        // Forward to PTY
        let bytes = key_to_bytes(key);
        self.pty_input_tx.send(bytes)?;
        Ok(true)
      }

      Mode::ModalWithBuffering | Mode::ModalGuestAlt => {
        // Handle AI modal input
        let consumed = self.handle_ai_input(key).await?;
        if !consumed {
          // Not consumed, forward to PTY
          let bytes = key_to_bytes(key);
          self.pty_input_tx.send(bytes)?;
        }
        Ok(true)
      }
    }
  }

  /// Handle AI modal input
  async fn handle_ai_input(&mut self, key: KeyEvent) -> Result<bool> {
    if let Some(ref mut ai) = self.ai_process {
      // If there's a pending command, handle approval
      if ai.pending_command().is_some() {
        match key.code {
          KeyCode::Char('y') | KeyCode::Char('Y') => {
            if let Some(pending) = ai.approve_command() {
              log::info!("Executing approved command: {}", pending.command);
              // Send command to PTY
              self
                .pty_input_tx
                .send(pending.command.as_bytes().to_vec())?;
              self.pty_input_tx.send(vec![b'\r'])?;
            }
            self.update_ai_modal();
            return Ok(true);
          }
          KeyCode::Char('n') | KeyCode::Char('N') => {
            ai.reject_command();
            log::info!("Command rejected");
            self.update_ai_modal();
            return Ok(true);
          }
          _ => return Ok(true), // Ignore other keys during approval
        }
      }

      // Normal input handling
      match key.code {
        KeyCode::Char(c) if key.modifiers.is_empty() => {
          ai.append_input(&c.to_string());
          self.update_ai_modal();
          Ok(true)
        }
        KeyCode::Backspace => {
          ai.delete_char();
          self.update_ai_modal();
          Ok(true)
        }
        KeyCode::Enter => {
          if !ai.input_buffer().is_empty() {
            log::info!("Sending message to AI");
            // Extract context first to avoid borrow issues
            let context = self.extract_context().await;
            // Now send with context - need to reborrow ai as mutable
            if let Some(ref mut ai) = self.ai_process {
              ai.send_input_with_context(context).await?;
              self.update_ai_modal();
            }
          }
          Ok(true)
        }
        _ => Ok(false), // Not consumed
      }
    } else {
      Ok(false)
    }
  }

  /// Toggle modal visibility
  async fn toggle_modal(&mut self) -> Result<()> {
    let is_visible = self.mode_manager.read().await.is_modal_visible();
    if is_visible {
      self.close_modal().await
    } else {
      self.show_modal().await
    }
  }

  /// Show modal
  async fn show_modal(&mut self) -> Result<()> {
    let transition = {
      let mut mode_mgr = self.mode_manager.write().await;
      mode_mgr.set_modal_visible(true)
    };

    // Sync host buffer
    self
      .output_router
      .synchronize_host_buffer(&transition)
      .await?;

    // Create renderer if needed
    {
      let mut renderer_guard = self.renderer.lock().unwrap();
      if renderer_guard.is_none() {
        let stdout = std::io::stdout();
        *renderer_guard = Some(RatatuiRenderer::new(stdout)?);
      }
    }

    // Update modal state
    self.update_ai_modal();

    // Force render
    self.render().await?;

    Ok(())
  }

  /// Close modal
  async fn close_modal(&mut self) -> Result<()> {
    let transition = {
      let mut mode_mgr = self.mode_manager.write().await;
      mode_mgr.set_modal_visible(false)
    };

    // Sync host buffer (includes replay if needed)
    self
      .output_router
      .synchronize_host_buffer(&transition)
      .await?;

    *self.modal_state.lock().unwrap() = None;

    Ok(())
  }

  /// Handle terminal resize
  async fn handle_resize(&mut self, cols: u16, rows: u16) -> Result<()> {
    // Resize PTY
    self.pty_bridge.resize(rows, cols)?;

    // Resize shadow terminal
    {
      let mut shadow = self.shadow_terminal.write().await;
      shadow.resize(cols, rows);
    }

    // Force render if in ratatui mode
    let mode = self.mode_manager.read().await.current_mode();
    if mode != Mode::Passthrough {
      self.render().await?;
    }

    Ok(())
  }

  /// Render the current state
  async fn render(&self) -> Result<()> {
    let mode = self.mode_manager.read().await.current_mode();

    if mode == Mode::Passthrough {
      // No ratatui rendering needed
      return Ok(());
    }

    // Get renderer
    let mut renderer_guard = self.renderer.lock().unwrap();
    if let Some(ref mut renderer) = *renderer_guard {
      // Get terminal content from shadow
      let content = {
        let shadow = self.shadow_terminal.read().await;
        shadow.visible_content()
      };

      // Get modal state
      let mut modal_guard = self.modal_state.lock().unwrap();
      let modal_ref = modal_guard.as_mut();

      // Render
      renderer.render_frame(&content, modal_ref)?;
    }

    Ok(())
  }

  async fn run(mut self) -> Result<()> {
    log::info!("Termin.AI starting main loop");

    loop {
      tokio::select! {
          // Handle PTY output
          Some(data) = self.pty_output_rx.recv() => {
              self.output_router.route_output(&data).await?;
          }

          // Render tick (60 FPS) - also handle keyboard input
          _ = tokio::time::sleep(Duration::from_millis(16)) => {
              // Check for shell exit
              if let Some(code) = self.pty_bridge.check_exit() {
                  log::info!("Shell exited with code: {}", code);
                  break;
              }

              // Check for keyboard events (non-blocking)
              if event::poll(Duration::from_millis(1))? {
                  if let CrosstermEvent::Key(key) = event::read()? {
                      if !self.handle_key_input(key).await? {
                          break;
                      }
                  } else if let CrosstermEvent::Resize(cols, rows) = event::read()? {
                      self.handle_resize(cols, rows).await?;
                  }
              }

              // Render if needed
              self.render().await?;
          }
      }
    }

    // Clean up
    let mut host = self.host_controller.lock().await;
    host.leave_alt_buffer()?;

    Ok(())
  }
}

// ============================================================================
// Main Entry Point
// ============================================================================

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

  flexi_logger::Logger::try_with_env_or_str("info,genai=debug,reqwest=debug")?
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

  // Detect user's shell
  let shell =
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());

  log::info!("Termin.AI starting with shell: {}", shell);

  // Create and run the app
  let app = App::new(shell).await?;
  app.run().await?;

  log::info!("Termin.AI exiting normally");
  Ok(())
}
