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
use tui::{Terminal, backend::Backend, layout::Rect};

// Import only what we need from the crate
use termin::ai_proc::{AIChatProcess, AIChatUI, ContextExtractor};
use termin::command::CommandExecutor;
use termin::encode_term::{KeyCodeEncodeModes, encode_key};
use termin::key::Key;
use termin::llm::Provider;
use termin::vt100;

// Shell events
#[derive(Debug)]
enum ShellEvent {
  Output,
  Exited(u32),
}

// Shell manager - simplified from mprocs' Inst
struct Shell {
  vt: Arc<RwLock<vt100::Parser<DummyReplySender>>>,
  writer: Box<dyn Write + Send>,
  _pid: u32,
  event_rx: UnboundedReceiver<ShellEvent>,
}

impl Shell {
  fn spawn(shell_cmd: &str, rows: u16, cols: u16) -> Result<Self> {
    log::info!("Spawning shell: {} ({}x{})", shell_cmd, cols, rows);

    // Create VT100 parser (borrowed from mprocs pattern)
    let vt = vt100::Parser::new(rows, cols, 1000, DummyReplySender);
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

    // Spawn command
    let mut child = pair.slave.spawn_command(cmd)?;
    let pid = child.process_id().unwrap_or(0);

    log::info!("Shell spawned with PID: {}", pid);

    // Get reader and writer for PTY
    let mut reader = pair.master.try_clone_reader()?;
    let writer = pair.master.take_writer()?;

    // Setup event channel
    let (event_tx, event_rx) = mpsc::unbounded_channel();

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
}

// Dummy reply sender (needed for VT100 parser)
#[derive(Clone)]
struct DummyReplySender;

impl termin::vt100::TermReplySender for DummyReplySender {
  fn reply(&self, _reply: compact_str::CompactString) {
    // Terminal reply sequences (like cursor position reports) would go here
    // For now, we ignore them
  }
}

#[tokio::main]
async fn main() -> Result<()> {
  // Setup logging
  flexi_logger::Logger::try_with_str("info")?
    .log_to_file(flexi_logger::FileSpec::default().suppress_timestamp())
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
  shell: Shell,
  ai_process: Option<AIChatProcess>,
  ai_visible: bool,
}

impl App {
  async fn new(shell_cmd: String) -> Result<Self> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;

    // Get terminal size
    let (cols, rows) = crossterm::terminal::size()?;

    // Spawn shell
    let shell = Shell::spawn(&shell_cmd, rows, cols)?;

    // Initialize AI if API key configured
    // Note: We still show the AI overlay even without a key,
    // but it will display a "not configured" message
    let ai_process = match std::env::var("ANTHROPIC_API_KEY") {
      Ok(_) => {
        log::info!("Initializing AI assistant");
        match AIChatProcess::new(Provider::Anthropic, None).await {
          Ok(ai) => {
            log::info!("AI assistant initialized successfully");
            Some(ai)
          }
          Err(e) => {
            log::warn!("Failed to initialize AI: {:?}", e);
            None
          }
        }
      }
      Err(_) => {
        log::info!(
          "No ANTHROPIC_API_KEY found - AI overlay will show config instructions"
        );
        None
      }
    };

    Ok(Self {
      shell,
      ai_process,
      ai_visible: false,
    })
  }

  async fn run(&mut self) -> Result<()> {
    log::info!("Termin.AI main loop starting");

    loop {
      tokio::select! {
        // Handle shell events
        Some(event) = self.shell.event_rx.recv() => {
          match event {
            ShellEvent::Output => {
              // Shell produced output, screen needs re-rendering
              // (VT100 parser has already processed it)
            }
            ShellEvent::Exited(code) => {
              log::info!("Shell exited with code: {}", code);
              break;
            }
          }
        }

        // Handle keyboard input
        _ = tokio::time::sleep(std::time::Duration::from_millis(16)) => {
          if event::poll(std::time::Duration::from_millis(1))? {
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
                } else {
                  // TODO: Route to AI overlay when visible
                  log::debug!("AI overlay input: {:?}", key);
                }
              }
              Event::Resize(cols, rows) => {
                log::info!("Terminal resized: {}x{}", cols, rows);
                // TODO: Resize PTY
              }
              _ => {}
            }
          }
        }
      }
    }

    Ok(())
  }
}

impl Drop for App {
  fn drop(&mut self) {
    // Cleanup terminal
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), LeaveAlternateScreen);
  }
}
