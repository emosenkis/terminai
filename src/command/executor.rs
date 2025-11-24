use anyhow::Result;
use crossterm::event::KeyCode;

use crate::{
  kernel::{kernel_message::ProcSender, proc::ProcId},
  key::Key,
  proc::msg::ProcCmd,
};

/// Command execution result
#[derive(Debug, Clone)]
pub struct ExecutionResult {
  pub success: bool,
  pub exit_code: Option<i32>,
  pub output: String,
}

/// Execute a command in a target process
pub struct CommandExecutor;

impl CommandExecutor {
  pub fn new() -> Self {
    Self
  }

  /// Convert a command string into a sequence of Key events
  pub fn command_to_keys(&self, command: &str) -> Vec<Key> {
    let mut keys = Vec::new();

    // Convert the command string into a sequence of key presses
    for ch in command.chars() {
      let key =
        Key::new(KeyCode::Char(ch), crossterm::event::KeyModifiers::NONE);
      keys.push(key);
    }

    // Add Enter key to execute the command
    let enter_key =
      Key::new(KeyCode::Enter, crossterm::event::KeyModifiers::NONE);
    keys.push(enter_key);

    keys
  }

  /// Send a command string to a process by converting it to key events
  pub fn send_command(
    &self,
    proc_sender: &ProcSender,
    command: &str,
  ) -> Result<()> {
    // Use the command_to_keys method
    for key in self.command_to_keys(command) {
      proc_sender.send(ProcCmd::SendKey(key));
    }

    Ok(())
  }

  /// Send a command to a specific process by ProcId
  pub fn send_command_to_proc(
    &self,
    proc_id: ProcId,
    proc_senders: &std::collections::HashMap<ProcId, ProcSender>,
    command: &str,
  ) -> Result<()> {
    if let Some(proc_sender) = proc_senders.get(&proc_id) {
      self.send_command(proc_sender, command)?;
      Ok(())
    } else {
      anyhow::bail!("Process {} not found", proc_id.0)
    }
  }
}

impl Default for CommandExecutor {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_executor_creation() {
    let _executor = CommandExecutor::new();
    // Basic test - just ensure it can be created
    assert!(true);
  }
}
