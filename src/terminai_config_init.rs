//! Helpers for creating editable Termin.AI config files.

use anyhow::Result;
use std::path::{Path, PathBuf};

pub const DEFAULT_TERMINAI_YAML: &str = r#"# Termin.AI configuration
#
# This file lives in the Termin.AI config directory. Edit it to choose the
# AI terminal agent, overlay behavior, providers, and custom agent presets.

interface:
  chat-position: bottom
  key_bindings:
    activate-overlay: "Ctrl-Space"
    deactivate-overlay: "Ctrl-Space"
    approve: "Y"
    deny: "N"

agent:
  preset: codex
  # extra-args:
  #   - --model
  #   - gpt-5

agent-presets: {}

providers:
  - name: openai
    display_name: OpenAI
    api_key_env: OPENAI_API_KEY
    models:
      - name: GPT-5
        model: gpt-5
  - name: anthropic
    display_name: Anthropic
    api_key_env: ANTHROPIC_API_KEY
    models:
      - name: Claude Sonnet 4.5
        model: claude-sonnet-4-5
  - name: gemini
    display_name: Gemini
    api_key_env: GEMINI_API_KEY
    models:
      - name: Gemini 2.5 Pro
        model: gemini-2.5-pro
  - name: openrouter
    display_name: OpenRouter
    api_key_env: OPENROUTER_API_KEY
    models: []
  - name: ollama
    display_name: Ollama
    models: []

default_model: openai/gpt-5
"#;

pub const DEFAULT_TERMINAI_ENV: &str = r#"# Termin.AI environment variables
#
# Add API keys here if you want Termin.AI to load them automatically.
# This file is created with owner-only permissions on Unix.

# OPENAI_API_KEY=
# ANTHROPIC_API_KEY=
# GEMINI_API_KEY=
# OPENROUTER_API_KEY=
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
  let xdg_dirs = xdg::BaseDirectories::with_prefix("terminai");
  xdg_dirs.get_config_home().ok_or_else(|| {
    anyhow::anyhow!("Failed to determine Termin.AI config directory")
  })
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
        .contains("OPENAI_API_KEY")
    );

    let _ = std::fs::remove_dir_all(result.config_dir);
  }

  #[test]
  fn default_terminai_yaml_is_parseable() {
    let config: crate::terminai_config::TerminAIConfig =
      serde_yaml::from_str(DEFAULT_TERMINAI_YAML).unwrap();

    assert_eq!(config.agent.preset.as_deref(), Some("codex"));
    assert_eq!(config.default_model, "openai/gpt-5");
    assert!(config.find_provider("openai").is_some());
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
