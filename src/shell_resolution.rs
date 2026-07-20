use anyhow::{Result, bail};
use std::path::Path;

use crate::terminai_config::ShellConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedShell {
  pub command: String,
  pub args: Vec<String>,
  /// Arguments before Terminai adds its CWD-reporting bootstrap.
  pub fallback_args: Vec<String>,
  pub identity: String,
  pub bootstrap: bool,
}

fn shell_identity(command: &str) -> Option<&'static str> {
  let name = Path::new(command)
    .file_name()?
    .to_string_lossy()
    .to_ascii_lowercase();
  match name.as_str() {
    "pwsh" | "pwsh.exe" => Some("pwsh.exe"),
    "powershell" | "powershell.exe" => Some("powershell.exe"),
    "cmd" | "cmd.exe" => Some("cmd.exe"),
    _ => None,
  }
}

fn is_resolvable(command: &str) -> bool {
  which::which(command).is_ok()
}

fn fallback_shell() -> Option<String> {
  #[cfg(windows)]
  {
    ["pwsh.exe", "powershell.exe", "cmd.exe"]
      .iter()
      .find(|command| is_resolvable(command))
      .map(|command| (*command).into())
  }
  #[cfg(not(windows))]
  {
    std::env::var("SHELL")
      .ok()
      .filter(|shell| is_resolvable(shell))
      .or_else(|| is_resolvable("/bin/bash").then_some("/bin/bash".into()))
  }
}

fn validate_bootstrap_args(
  identity: Option<&str>,
  args: &[String],
) -> Result<()> {
  let invalid = match identity {
    Some("pwsh.exe" | "powershell.exe") => {
      ["-command", "-file", "-encodedcommand"].as_slice()
    }
    Some("cmd.exe") => ["/c", "/k"].as_slice(),
    _ => return Ok(()),
  };
  if args
    .iter()
    .any(|arg| invalid.iter().any(|flag| arg.eq_ignore_ascii_case(flag)))
  {
    bail!(
      "shell arguments cannot include an execution-mode flag when Terminai installs CWD reporting"
    )
  }
  Ok(())
}

pub fn powershell_bootstrap() -> String {
  "$__terminai_prompt = ${function:prompt}; function global:prompt { $p = (Get-Location).Path -replace '\\\\','/'; $u = [uri]::EscapeUriString($p); [Console]::Write(\"`e]7;file:///$u`a\"); & $__terminai_prompt }".into()
}

pub fn cmd_bootstrap() -> String {
  "set \"PROMPT=$E]7;file:///$P$E\\$G$PROMPT\"".into()
}

fn with_bootstrap(
  command: String,
  mut args: Vec<String>,
) -> Result<ResolvedShell> {
  let fallback_args = args.clone();
  let identity = shell_identity(&command).map(str::to_string);
  validate_bootstrap_args(identity.as_deref(), &args)?;
  let bootstrap = identity.is_some();
  if let Some(identity) = identity.as_deref() {
    match identity {
      "pwsh.exe" | "powershell.exe" => args.extend([
        "-NoExit".into(),
        "-Command".into(),
        powershell_bootstrap(),
      ]),
      "cmd.exe" => args.extend(["/K".into(), cmd_bootstrap()]),
      _ => {}
    }
  }
  Ok(ResolvedShell {
    identity: identity.unwrap_or_else(|| command.clone()),
    command,
    args,
    fallback_args,
    bootstrap,
  })
}

pub fn resolve_shell(
  explicit: &[String],
  env_shell: Option<String>,
  configured: Option<&ShellConfig>,
  parent_shell: Option<String>,
) -> Result<ResolvedShell> {
  if let Some(command) = explicit.first() {
    return with_bootstrap(command.clone(), explicit[1..].to_vec());
  }
  let selected = if let Some(command) = env_shell {
    Some((command, Vec::new(), true))
  } else if let Some(shell) = configured.filter(|shell| shell.command.is_some())
  {
    Some((shell.command.clone().unwrap(), shell.args.clone(), true))
  } else if let Some(command) =
    parent_shell.filter(|command| shell_identity(command).is_some())
  {
    Some((command, Vec::new(), false))
  } else {
    fallback_shell().map(|command| (command, Vec::new(), false))
  };
  let Some((command, args, required)) = selected else {
    bail!("could not find a usable default shell")
  };
  if required && !is_resolvable(&command) {
    bail!("selected shell `{command}` cannot be resolved")
  }
  with_bootstrap(command, args)
}

#[cfg(windows)]
pub fn parent_shell() -> Option<String> {
  use std::collections::HashMap;
  use std::mem::{size_of, zeroed};
  use winapi::um::handleapi::CloseHandle;
  use winapi::um::processthreadsapi::GetCurrentProcessId;
  use winapi::um::tlhelp32::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
    TH32CS_SNAPPROCESS,
  };
  unsafe {
    let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if snapshot.is_null() || snapshot == (-1isize) as _ {
      return None;
    }
    let mut entries = HashMap::new();
    let mut item: PROCESSENTRY32W = zeroed();
    item.dwSize = size_of::<PROCESSENTRY32W>() as u32;
    if Process32FirstW(snapshot, &mut item) != 0 {
      loop {
        let name = String::from_utf16_lossy(&item.szExeFile)
          .trim_end_matches('\0')
          .to_string();
        entries.insert(item.th32ProcessID, (item.th32ParentProcessID, name));
        item.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        if Process32NextW(snapshot, &mut item) == 0 {
          break;
        }
      }
    }
    CloseHandle(snapshot);
    let mut pid = GetCurrentProcessId();
    while let Some((parent, name)) = entries.get(&pid) {
      if shell_identity(name).is_some() {
        return Some(name.clone());
      };
      pid = *parent;
    }
  }
  None
}

#[cfg(not(windows))]
pub fn parent_shell() -> Option<String> {
  None
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn explicit_has_precedence() {
    let got = resolve_shell(
      &["custom.exe".into()],
      Some("other.exe".into()),
      None,
      None,
    )
    .unwrap();
    assert_eq!(got.command, "custom.exe");
  }
  #[test]
  fn config_execution_flags_are_rejected() {
    let shell = ShellConfig {
      command: Some("cmd.exe".into()),
      args: vec!["/C".into()],
    };
    assert!(resolve_shell(&[], None, Some(&shell), None).is_err());
  }
  #[test]
  fn bootstrap_payloads_emit_osc7() {
    assert!(powershell_bootstrap().contains("]7;file:///"));
    assert!(cmd_bootstrap().contains("$P"));
  }

  #[test]
  fn cmd_bootstrap_terminates_osc7_before_the_prompt() {
    assert_eq!(
      cmd_bootstrap(),
      "set \"PROMPT=$E]7;file:///$P$E\\$G$PROMPT\""
    );
  }
}
