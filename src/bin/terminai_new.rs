// Termin.AI - Single-shell terminal with AI overlay using Hybrid Terminal System
// Uses the sophisticated hybrid terminal for proper mode management and output buffering

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::Write;
use std::sync::{Arc, RwLock as StdRwLock};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

// Import hybrid terminal system
use termin::hybrid::{
    AppEvent, HybridTerminal, ModalContent, ModalState,
};

// Import AI and other components
use termin::ai_proc::AIChatProcess;
use termin::llm::{Provider, TerminalContext};
use termin::vt100::TermReplySender;

// ============================================================================
// PTY Bridge - Manages PTY and channels for HybridTerminal
// ============================================================================

struct PtyBridge {
    master: Box<dyn portable_pty::MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    exit_code: Arc<StdRwLock<Option<u32>>>,
}

impl PtyBridge {
    /// Spawn a shell PTY and return bridge + channels for HybridTerminal
    fn spawn(
        shell_cmd: &str,
        rows: u16,
        cols: u16,
    ) -> Result<(
        Self,
        UnboundedReceiver<Vec<u8>>, // PTY output for HybridTerminal
        UnboundedSender<Vec<u8>>,   // PTY input from HybridTerminal
    )> {
        log::info!("Spawning shell: {} ({}x{})", shell_cmd, cols, rows);

        // Create channels
        let (output_tx, output_rx) = mpsc::unbounded_channel();
        let (input_tx, mut input_rx) = mpsc::unbounded_channel();

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
                        // Send to HybridTerminal via channel
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
                writer,
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
// Terminal Reply Sender - Implementation for HybridTerminal
// ============================================================================

#[derive(Clone)]
struct TerminalReplySender {
    pty_input_tx: UnboundedSender<Vec<u8>>,
}

impl TermReplySender for TerminalReplySender {
    fn send(&self, data: Vec<u8>) {
        // Send terminal query replies back to PTY
        let _ = self.pty_input_tx.send(data);
    }
}

// ============================================================================
// AI Modal Adapter - Converts AIChatProcess to ModalState
// ============================================================================

struct AIModalAdapter {
    ai_process: AIChatProcess,
    pty_input_tx: UnboundedSender<Vec<u8>>,
}

impl AIModalAdapter {
    /// Convert AI state to modal state for rendering
    fn to_modal_state(&self) -> ModalState {
        let mut content = String::new();

        // Show conversation history
        for msg in self.ai_process.conversation() {
            match msg.role {
                termin::ai_proc::MessageRole::User => {
                    content.push_str(&format!("You: {}\n\n", msg.content));
                }
                termin::ai_proc::MessageRole::Assistant => {
                    content.push_str(&format!("AI: {}\n\n", msg.content));
                }
                termin::ai_proc::MessageRole::System => {
                    // Skip system messages in display
                }
            }
        }

        // Show input buffer
        let input = self.ai_process.input_buffer();
        if !input.is_empty() || content.is_empty() {
            content.push_str(&format!("\n> {}_", input));
        }

        // Show pending command approval if any
        if let Some(pending) = self.ai_process.pending_command() {
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

        ModalState::text("AI Assistant (Ctrl+Space to toggle)", content)
    }

    /// Handle keyboard input for AI modal
    async fn handle_input(&mut self, key: KeyEvent) -> Result<AIInputResult> {
        // If there's a pending command, handle approval
        if self.ai_process.pending_command().is_some() {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.execute_approved_command()?;
                    return Ok(AIInputResult::Consumed);
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.ai_process.reject_command();
                    log::info!("Command rejected by user");
                    return Ok(AIInputResult::Consumed);
                }
                _ => return Ok(AIInputResult::Consumed), // Ignore other keys during approval
            }
        }

        // Normal input handling
        match key.code {
            KeyCode::Char(c) if key.modifiers.is_empty() => {
                self.ai_process.append_input(&c.to_string());
                Ok(AIInputResult::Consumed)
            }
            KeyCode::Backspace => {
                self.ai_process.delete_char();
                Ok(AIInputResult::Consumed)
            }
            KeyCode::Enter => {
                if !self.ai_process.input_buffer().is_empty() {
                    Ok(AIInputResult::SendMessage)
                } else {
                    Ok(AIInputResult::Consumed)
                }
            }
            KeyCode::Esc => Ok(AIInputResult::CloseModal),
            _ => Ok(AIInputResult::PassThrough),
        }
    }

    /// Send message to AI with terminal context
    async fn send_message(&mut self, context: TerminalContext) -> Result<()> {
        log::info!("Sending message to AI with context");
        self.ai_process.send_input_with_context(context).await?;
        log::info!("Message sent successfully");
        Ok(())
    }

    /// Execute approved command by sending to PTY
    fn execute_approved_command(&mut self) -> Result<()> {
        if let Some(pending) = self.ai_process.approve_command() {
            log::info!("Executing approved command: {}", pending.command);

            // Send command to PTY
            self.pty_input_tx.send(pending.command.as_bytes().to_vec())?;

            // Send Enter to execute
            self.pty_input_tx.send(vec![b'\r'])?;

            log::info!("Command sent to shell");
        }
        Ok(())
    }
}

enum AIInputResult {
    Consumed,
    PassThrough,
    CloseModal,
    SendMessage,
}

// ============================================================================
// Main App - Coordinates HybridTerminal and AI
// ============================================================================

struct App {
    hybrid_terminal: HybridTerminal<TerminalReplySender>,
    ai_adapter: Option<AIModalAdapter>,
    pty_bridge: PtyBridge,
    app_event_tx: UnboundedSender<AppEvent>,
    pty_input_tx: UnboundedSender<Vec<u8>>,
}

impl App {
    async fn new(shell_cmd: String) -> Result<Self> {
        // Get terminal size
        let (cols, rows) = crossterm::terminal::size()?;

        // Create PTY bridge
        let (pty_bridge, pty_output_rx, pty_input_tx) =
            PtyBridge::spawn(&shell_cmd, rows, cols)?;

        // Create app event channel
        let (app_event_tx, app_event_rx) = mpsc::unbounded_channel();

        // Create reply sender for vt100 terminal queries
        let reply_sender = TerminalReplySender {
            pty_input_tx: pty_input_tx.clone(),
        };

        // Create hybrid terminal
        let mut hybrid_terminal = HybridTerminal::new(
            cols,
            rows,
            1000, // scrollback lines
            reply_sender,
            pty_output_rx,
            pty_input_tx.clone(),
            app_event_rx,
        )?;

        // Initialize AI if API key available
        let ai_adapter = Self::init_ai(pty_input_tx.clone()).await;

        Ok(Self {
            hybrid_terminal,
            ai_adapter,
            pty_bridge,
            app_event_tx,
            pty_input_tx,
        })
    }

    /// Initialize AI assistant if API keys are available
    async fn init_ai(pty_input_tx: UnboundedSender<Vec<u8>>) -> Option<AIModalAdapter> {
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
                        return Some(AIModalAdapter {
                            ai_process,
                            pty_input_tx,
                        });
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

    async fn run(mut self) -> Result<()> {
        log::info!("Termin.AI starting main loop");

        // Run hybrid terminal in the background
        let hybrid_handle = tokio::spawn(async move {
            if let Err(e) = self.hybrid_terminal.run().await {
                log::error!("Hybrid terminal error: {}", e);
            }
        });

        // Monitor for shell exit
        loop {
            if let Some(code) = self.pty_bridge.check_exit() {
                log::info!("Shell exited with code: {}", code);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        // Send quit event to hybrid terminal
        let _ = self.app_event_tx.send(AppEvent::Quit);

        // Wait for hybrid terminal to finish
        let _ = hybrid_handle.await;

        Ok(())
    }
}

// ============================================================================
// Main Entry Point
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    // Setup logging
    flexi_logger::Logger::try_with_str("info")?
        .log_to_file(flexi_logger::FileSpec::default().suppress_timestamp())
        .start()?;

    // Detect user's shell
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());

    log::info!("Termin.AI starting with shell: {}", shell);

    // Create and run the app
    let app = App::new(shell).await?;
    app.run().await?;

    log::info!("Termin.AI exiting normally");
    Ok(())
}
