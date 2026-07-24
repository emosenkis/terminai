//! Helpers for creating editable Terminai config files.

use anyhow::Result;
use std::path::{Path, PathBuf};

pub const DEFAULT_TERMINAI_YAML: &str = concat!(
  "# yaml-language-server: $schema=https://terminai.app/schema-v",
  env!("CARGO_PKG_VERSION"),
  r#".json
# Terminai configuration
#
# This file lives in the Terminai config directory. Edit it to choose the
# AI terminal agent, overlay behavior, and custom agent presets.

interface:
  chat-position: bottom
  chat-height-percent: 50
  guest-display: resize
  key_bindings:
    activate-overlay: "Ctrl-Space"
    deactivate-overlay: "Ctrl-Space"
    approve: "Y"
    deny: "N"
    layout-mode: "F9"
    control-panel: "F10"
    toggle-fullscreen: "F11"

# DANGER: auto-approval sends every AI suggestion directly to the shell
# without consulting the command risk classifier.
approval-mode: always-ask

# Terminal text returned to an AI agent is filtered with Redact's
# pattern-based recognizers. `default` redacts credentials and strong personal
# identifiers while retaining URLs, IPs, timestamps, postal codes, and
# technical diagnostics. Add a category or entity type, or remove one with
# a leading `-`, e.g. [default, -btc-address].
privacy:
  patterns: [default]
  strategy: replace

# Optional default shell (pwsh.exe, powershell.exe, or cmd.exe on Windows).
# shell:
#   command: pwsh.exe
#   args: ["-NoLogo"]

agent:
  preset: codex
  # prompt-template: custom.jinja
  # Templates are loaded from this directory. A default.jinja here shadows
  # the bundled default; it can extend "builtin/default.jinja".
  # extra-args:
  #   - --model
  #   - gpt-5
  #   - expr: '["--search"] if uses_mcp else []'

# Built-in agent preset reference configs are bundled from:
# - config/codex.yaml
# - config/claude.yaml
# - config/opencode.yaml
# - config/default.jinja
# Add overrides or new presets here using the same shape.
# Set show-in-switcher: false on a user preset to hide it from the picker.
agent-presets: {}
"#,
);

pub const DEFAULT_TERMINAI_ENV: &str = r#"# Terminai environment variables
#
# Add environment variables here if you want Terminai to load them automatically.
# This file is created with owner-only permissions on Unix.

"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigInitAction {
  Written,
  Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigInitFile {
  pub path: PathBuf,
  pub action: ConfigInitAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigInitResult {
  pub config_dir: PathBuf,
  pub files: Vec<ConfigInitFile>,
}

pub fn terminai_config_dir() -> Result<PathBuf> {
  crate::paths::config_dir()
}

pub fn init_config_files(force: bool) -> Result<ConfigInitResult> {
  init_config_files_in(terminai_config_dir()?, force)
}

pub fn init_config_files_in(
  config_dir: PathBuf,
  force: bool,
) -> Result<ConfigInitResult> {
  std::fs::create_dir_all(&config_dir)?;

  let files = vec![
    write_config_file(
      &config_dir.join("terminai.yaml"),
      DEFAULT_TERMINAI_YAML,
      force,
      None,
    )?,
    write_config_file(
      &config_dir.join("terminai.env"),
      DEFAULT_TERMINAI_ENV,
      force,
      Some(0o600),
    )?,
  ];

  Ok(ConfigInitResult { config_dir, files })
}

fn write_config_file(
  path: &Path,
  contents: &str,
  force: bool,
  unix_mode: Option<u32>,
) -> Result<ConfigInitFile> {
  if path.exists() && !force {
    return Ok(ConfigInitFile {
      path: path.to_path_buf(),
      action: ConfigInitAction::Skipped,
    });
  }

  write_file(path, contents, unix_mode)?;
  Ok(ConfigInitFile {
    path: path.to_path_buf(),
    action: ConfigInitAction::Written,
  })
}

#[cfg(unix)]
fn write_file(
  path: &Path,
  contents: &str,
  unix_mode: Option<u32>,
) -> Result<()> {
  use std::io::Write;
  use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

  let mut options = std::fs::OpenOptions::new();
  options.write(true).create(true).truncate(true);
  if let Some(mode) = unix_mode {
    options.mode(mode);
  }

  let mut file = options.open(path)?;
  file.write_all(contents.as_bytes())?;
  if let Some(mode) = unix_mode {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
  }
  Ok(())
}

#[cfg(not(unix))]
fn write_file(
  path: &Path,
  contents: &str,
  _unix_mode: Option<u32>,
) -> Result<()> {
  std::fs::write(path, contents)?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn init_config_files_creates_full_default_set() {
    let dir = std::env::temp_dir()
      .join(format!("terminai-config-init-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    let result = init_config_files_in(dir.clone(), false).unwrap();

    assert_eq!(result.config_dir, dir);
    assert_eq!(result.files.len(), 2);
    assert!(dir.join("terminai.yaml").exists());
    assert!(dir.join("terminai.env").exists());
    assert!(
      std::fs::read_to_string(dir.join("terminai.yaml"))
        .unwrap()
        .contains("agent:")
    );
    assert!(
      std::fs::read_to_string(dir.join("terminai.env"))
        .unwrap()
        .contains("environment variables")
    );

    let _ = std::fs::remove_dir_all(result.config_dir);
  }

  #[test]
  fn default_terminai_yaml_is_parseable() {
    let config: crate::terminai_config::TerminaiConfig =
      serde_yaml::from_str(DEFAULT_TERMINAI_YAML).unwrap();

    assert_eq!(config.agent.preset.as_deref(), Some("codex"));
  }

  #[test]
  fn init_config_files_does_not_overwrite_without_force() {
    let dir = std::env::temp_dir().join(format!(
      "terminai-config-init-skip-test-{}",
      std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("terminai.yaml"), "custom: true\n").unwrap();

    let result = init_config_files_in(dir.clone(), false).unwrap();

    assert_eq!(
      std::fs::read_to_string(dir.join("terminai.yaml")).unwrap(),
      "custom: true\n"
    );
    assert_eq!(result.files[0].action, ConfigInitAction::Skipped);
    assert_eq!(result.files[1].action, ConfigInitAction::Written);

    let _ = std::fs::remove_dir_all(result.config_dir);
  }
}
