// TermCap - Terminal Capture Utility
// Runs a command in a headless terminal and prints the final output
// Useful for testing terminal applications

use anyhow::Result;
use clap::Parser;
use std::io::Write;
use termin::shell::{Shell, ShellEvent};
use tui::Terminal;
use tui::backend::TestBackend;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
  /// Command to run
  #[arg(required = true)]
  command: Vec<String>,

  /// Terminal width
  #[arg(long, default_value_t = 80)]
  width: u16,

  /// Terminal height
  #[arg(long, default_value_t = 24)]
  height: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
  // Setup logging
  flexi_logger::Logger::try_with_env_or_str("info")?.start()?;

  let args = Args::parse();

  if args.command.is_empty() {
    anyhow::bail!("No command specified");
  }

  let cmd = &args.command[0];
  let cmd_args = &args.command[1..];

  log::info!("Starting TermCap with command: {} {:?}", cmd, cmd_args);

  // Initialize TestBackend
  let backend = TestBackend::new(args.width, args.height);
  let mut terminal = Terminal::new(backend)?;

  // Spawn command
  let mut shell =
    Shell::spawn_command(cmd, &cmd_args.to_vec(), args.height, args.width)?;

  // Event loop
  loop {
    tokio::select! {
        Some(event) = shell.event_rx.recv() => {
            match event {
                ShellEvent::Output => {
                    // Shell produced output - update terminal buffer
                    if let Ok(vt) = shell.vt.read() {
                        let screen = vt.screen();

                        // Render to TestBackend
                        terminal.draw(|f| {
                            let area = f.area();
                            for row in 0..area.height {
                                for col in 0..area.width {
                                    let pos = tui::layout::Position {
                                        x: area.x + col,
                                        y: area.y + row,
                                    };

                                    if let Some(to_cell) = f.buffer_mut().cell_mut(pos) {
                                        if let Some(cell) = screen.cell(row, col) {
                                            *to_cell = cell.to_tui();
                                            if !cell.has_contents() {
                                                to_cell.set_char(' ');
                                            }
                                        } else {
                                            to_cell.set_char(' ');
                                        }
                                    }
                                }
                            }
                        })?;
                    }
                }
                ShellEvent::TermReply(reply) => {
                    if let Err(e) = shell.writer.write_all(reply.as_bytes()) {
                        log::error!("Failed to write terminal reply: {:?}", e);
                    }
                    if let Err(e) = shell.writer.flush() {
                        log::error!("Failed to flush terminal reply: {:?}", e);
                    }
                }
                ShellEvent::Exited(code) => {
                    log::info!("Shell exited with code: {}", code);
                    break;
                }
            }
        }
        // Timeout or other events could be handled here
    }
  }

  // Print final buffer content to stdout
  let buffer = terminal.backend().buffer();
  for y in 0..args.height {
    let mut line = String::new();
    for x in 0..args.width {
      let pos = tui::layout::Position { x, y };
      if let Some(cell) = buffer.cell(pos) {
        line.push_str(cell.symbol());
      } else {
        line.push(' ');
      }
    }
    println!("{}", line);
  }

  Ok(())
}
