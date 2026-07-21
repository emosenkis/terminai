use anyhow::Result;

use crate::shell::{Shell, ShellEvent, ShellSpawnOptions};

pub struct AgentTerminal {
  shell: Shell,
}

impl AgentTerminal {
  pub fn spawn(
    command: &str,
    args: &[String],
    rows: u16,
    cols: u16,
    options: ShellSpawnOptions,
  ) -> Result<(Self, tokio::sync::mpsc::UnboundedReceiver<ShellEvent>)> {
    let (shell, rx) =
      Shell::spawn_command_with_options(command, args, rows, cols, options)?;
    Ok((Self { shell }, rx))
  }

  pub fn shell(&self) -> &Shell {
    &self.shell
  }

  pub fn shell_mut(&mut self) -> &mut Shell {
    &mut self.shell
  }

  pub fn terminate(&mut self) -> Result<()> {
    self.shell.terminate()
  }
}
