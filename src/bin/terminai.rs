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
use std::io::{Write, stdout};
use tokio::sync::mpsc;
use tui::{Terminal, backend::Backend, layout::Rect};

// Import only what we need from the crate
use termin::ai_proc::{AIChatProcess, AIChatUI, ContextExtractor};
use termin::command::CommandExecutor;
use termin::config::{CmdConfig, ProcConfig};
use termin::llm::Provider;
use termin::proc::StopSignal;
use termin::vt100::Size;

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
  ai_process: Option<AIChatProcess>,
  ai_visible: bool,
  shell_command: String,
  // TODO: Add shell process handle
}

impl App {
  async fn new(shell: String) -> Result<Self> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;

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
      ai_process,
      ai_visible: false,
      shell_command: shell,
    })
  }

  async fn run(&mut self) -> Result<()> {
    // Print welcome message
    println!("Termin.AI starting...");
    println!("Press Ctrl-Space to toggle AI overlay, Ctrl-C to quit");
    println!("Shell: {}", self.shell_command);
    if self.ai_process.is_some() {
      println!("AI: Configured and ready");
    } else {
      println!("AI: Not configured (set ANTHROPIC_API_KEY to enable LLM)");
      println!("    You can still open the AI overlay to see the interface");
    }

    loop {
      // Handle input
      if event::poll(std::time::Duration::from_millis(100))? {
        match event::read()? {
          Event::Key(KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            ..
          }) => {
            // Ctrl-C: exit
            break;
          }
          Event::Key(KeyEvent {
            code: KeyCode::Char(' '),
            modifiers: KeyModifiers::CONTROL,
            ..
          }) => {
            // Ctrl-Space: toggle AI overlay (works even without API key)
            self.ai_visible = !self.ai_visible;
            println!("\nAI overlay toggled: {}", self.ai_visible);
            log::info!("AI overlay toggled: {}", self.ai_visible);
          }
          Event::Key(KeyEvent {
            code: KeyCode::Esc, ..
          }) => {
            // ESC: close AI overlay
            if self.ai_visible {
              self.ai_visible = false;
              println!("\nAI overlay closed");
            }
          }
          event => {
            // TODO: Route to shell or AI depending on mode
            log::debug!("Input event: {:?}", event);
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
