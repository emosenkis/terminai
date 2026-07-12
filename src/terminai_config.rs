use std::{collections::HashMap, path::PathBuf};

use anyhow::Result;
use crokey::KeyCombination;
use crokey::key;
#[cfg(feature = "schema")]
use rmcp::schemars::{
  self as schemars, JsonSchema, Schema, SchemaGenerator, json_schema,
};
use serde::{Deserialize, Serialize};
#[cfg(feature = "schema")]
use std::borrow::Cow;

/// Position of the AI chat overlay
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schema", schemars(deny_unknown_fields))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatPosition {
  Bottom,
  Top,
}

impl Default for ChatPosition {
  fn default() -> Self {
    Self::Bottom
  }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum OneOrMoreBindings {
  Single(KeyCombination),
  Multiple(Vec<KeyCombination>),
}

#[cfg(feature = "schema")]
impl JsonSchema for OneOrMoreBindings {
  fn schema_name() -> Cow<'static, str> {
    "OneOrMoreBindings".into()
  }

  fn schema_id() -> Cow<'static, str> {
    concat!(module_path!(), "::OneOrMoreBindings").into()
  }

  fn json_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
      "oneOf": [
        {
          "type": "string",
          "description": "A single key combination such as Ctrl-Space"
        },
        {
          "type": "array",
          "items": {
            "type": "string"
          }
        }
      ]
    })
  }
}

impl OneOrMoreBindings {
  pub fn matches(&self, key_combo: KeyCombination) -> bool {
    match self {
      OneOrMoreBindings::Single(key) => *key == key_combo,
      OneOrMoreBindings::Multiple(keys) => keys.contains(&key_combo),
    }
  }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schema", schemars(deny_unknown_fields))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeyBindingsConfig {
  #[serde(rename = "activate-overlay")]
  pub activate_overlay: OneOrMoreBindings,
  #[serde(rename = "deactivate-overlay")]
  pub deactivate_overlay: OneOrMoreBindings,
  pub approve: OneOrMoreBindings,
  pub deny: OneOrMoreBindings,
}

impl Default for KeyBindingsConfig {
  fn default() -> Self {
    Self {
      activate_overlay: OneOrMoreBindings::Single(key!(ctrl - space)),
      deactivate_overlay: OneOrMoreBindings::Single(key!(ctrl - space)),
      approve: OneOrMoreBindings::Single(key!(y)),
      deny: OneOrMoreBindings::Single(key!(n)),
    }
  }
}

/// Interface configuration
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schema", schemars(deny_unknown_fields))]
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct InterfaceConfig {
  /// Position of the AI chat overlay (default: bottom)
  #[serde(default, rename = "chat-position")]
  pub chat_position: ChatPosition,
  /// Key bindings
  ///
  /// The syntax for key combinations is defined by [crokey](https://github.com/Canop/crokey).
  #[serde(default)]
  pub key_bindings: KeyBindingsConfig,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schema", schemars(deny_unknown_fields))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentKind {
  Claude,
  Codex,
  Custom,
}

/// Agent configuration.
///
/// The `args` and `extra-args` fields are rendered as Handlebars templates
/// when Terminai builds the launch command. Available variables and helpers:
/// `{{cwd}}`, `{{mcp_url}}`, `{{toml mcp_url}}`,
/// `{{terminai_mcp_command}}`, `{{toml terminai_mcp_command}}`,
/// `{{json terminai_mcp_command}}`, `{{terminai_mcp_port}}`,
/// `{{toml terminai_mcp_port}}`, `{{json terminai_mcp_port}}`,
/// `{{terminai_mcp_auth_token}}`, `{{toml terminai_mcp_auth_token}}`,
/// `{{json terminai_mcp_auth_token}}`,
/// `{{context_prompt}}`, `{{toml context_prompt}}`, `{{#args}}...{{/args}}`,
/// `{{#arg}}...{{/arg}}`, and `{{OMIT}}`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schema", schemars(deny_unknown_fields))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentConfig {
  #[serde(default)]
  pub preset: Option<String>,
  #[serde(default)]
  pub kind: Option<AgentKind>,
  #[serde(default)]
  pub command: Option<String>,
  /// CLI arguments passed to the agent. Supports the same Handlebars
  /// template variables and helpers documented on [`AgentConfig`].
  #[serde(default)]
  pub args: Vec<String>,
  /// Additional CLI arguments appended after `args`. Supports the same
  /// Handlebars template variables and helpers documented on [`AgentConfig`].
  #[serde(default, rename = "extra-args")]
  pub extra_args: Vec<String>,
  /// Initial prompt passed to the agent. See the default value in
  /// [`config/general.yaml`](https://github.com/emosenkis/terminai/blob/main/config/general.yaml).
  #[serde(default, rename = "initial-prompt")]
  pub initial_prompt: Option<String>,
}

impl AgentConfig {
  pub fn claude() -> Self {
    Self {
      preset: Some("claude".to_string()),
      kind: Some(AgentKind::Claude),
      command: None,
      args: Vec::new(),
      extra_args: Vec::new(),
      initial_prompt: None,
    }
  }

  pub fn codex() -> Self {
    Self {
      preset: Some("codex".to_string()),
      kind: Some(AgentKind::Codex),
      command: None,
      args: Vec::new(),
      extra_args: Vec::new(),
      initial_prompt: None,
    }
  }

  pub fn effective_kind(&self) -> AgentKind {
    if let Some(kind) = self.kind {
      return kind;
    }
    match self.command.as_deref() {
      Some("claude") => AgentKind::Claude,
      Some("codex") | None => AgentKind::Codex,
      Some(_) => AgentKind::Custom,
    }
  }
}

impl Default for AgentConfig {
  fn default() -> Self {
    Self::codex()
  }
}

/// Agent preset configuration.
///
/// The `args` and `extra-args` fields are rendered as Handlebars templates
/// when Terminai builds the launch command. Available variables and helpers:
/// `{{cwd}}`, `{{mcp_url}}`, `{{toml mcp_url}}`,
/// `{{terminai_mcp_command}}`, `{{toml terminai_mcp_command}}`,
/// `{{json terminai_mcp_command}}`, `{{terminai_mcp_port}}`,
/// `{{toml terminai_mcp_port}}`, `{{json terminai_mcp_port}}`,
/// `{{terminai_mcp_auth_token}}`, `{{toml terminai_mcp_auth_token}}`,
/// `{{json terminai_mcp_auth_token}}`,
/// `{{context_prompt}}`, `{{toml context_prompt}}`, `{{#args}}...{{/args}}`,
/// `{{#arg}}...{{/arg}}`, and `{{OMIT}}`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schema", schemars(deny_unknown_fields))]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct AgentPresetConfig {
  #[serde(default)]
  pub extends: Option<String>,
  #[serde(default)]
  pub command: Option<String>,
  #[serde(default)]
  pub args: Vec<String>,
  #[serde(default, rename = "extra-args")]
  pub extra_args: Vec<String>,
  #[serde(default)]
  pub env: HashMap<String, String>,
}

/// Top-level Terminai configuration loaded from
/// `$XDG_CONFIG_HOME/terminai/terminai.yaml`, falling back to
/// `~/.config/terminai/terminai.yaml` when `XDG_CONFIG_HOME` is unset.
///
/// Default configuration can be installed with `terminai init-config`
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schema", schemars(deny_unknown_fields))]
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct TerminaiConfig {
  /// Interface configuration
  #[serde(default)]
  pub interface: InterfaceConfig,
  /// External CLI agent to run in the AI terminal.
  #[serde(default)]
  pub agent: AgentConfig,
  /// User-defined CLI agent presets. Built-in presets include codex and claude.
  #[serde(default, rename = "agent-presets")]
  pub agent_presets: HashMap<String, AgentPresetConfig>,
}

impl TerminaiConfig {
  pub fn path() -> Result<PathBuf> {
    let config_dir = xdg::BaseDirectories::with_prefix("terminai");
    config_dir.find_config_file("terminai.yaml").ok_or_else(|| {
      // Build expected path for error message
      let expected_path = config_dir
        .get_config_home()
        .map(|p| p.join("terminai.yaml"))
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "~/.config/terminai/terminai.yaml".to_string());
      anyhow::anyhow!(
        "Configuration file not found. Expected at: {}",
        expected_path
      )
    })
  }

  pub fn expected_path() -> Result<PathBuf> {
    let config_dir = xdg::BaseDirectories::with_prefix("terminai");
    config_dir
      .get_config_home()
      .ok_or_else(|| {
        anyhow::anyhow!("Failed to determine Terminai config directory")
      })
      .map(|path| path.join("terminai.yaml"))
  }

  /// Load configuration from XDG config directory (~/.config/terminai/terminai.yaml)
  pub fn load() -> Result<Self> {
    let config_path = Self::path()?;

    log::info!("Loading configuration from: {}", config_path.display());
    let config_content = std::fs::read_to_string(&config_path)?;
    // TODO: Switch to HJSON? It's simpler and safer than YAML
    let config: TerminaiConfig = serde_yaml::from_str(&config_content)?;

    log::info!("Terminai configuration loaded");

    Ok(config)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_terminai_config_yaml_deserialize() {
    let yaml = r#"
agent:
  preset: claude
    "#;

    let config: TerminaiConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.agent.preset.as_deref(), Some("claude"));
    // Interface defaults to bottom when not specified
    assert_eq!(config.interface.chat_position, ChatPosition::Bottom);
  }

  #[test]
  fn test_agent_config_extra_args_and_user_presets() {
    let yaml = r#"
agent:
  preset: codex
  extra-args:
    - --model
    - gpt-5.5
agent-presets:
  opencode-fast:
    extends: opencode
    extra-args:
      - --model
      - github-copilot/gpt-5
    "#;

    let config: TerminaiConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.agent.preset.as_deref(), Some("codex"));
    assert_eq!(config.agent.extra_args, vec!["--model", "gpt-5.5"]);
    assert_eq!(
      config
        .agent_presets
        .get("opencode-fast")
        .unwrap()
        .extends
        .as_deref(),
      Some("opencode")
    );
  }

  #[test]
  fn test_default_deactivate_overlay_does_not_capture_escape() {
    let bindings = KeyBindingsConfig::default();

    assert!(bindings.deactivate_overlay.matches(key!(ctrl - space)));
    assert!(!bindings.deactivate_overlay.matches(key!(esc)));
  }

  #[test]
  fn test_deactivate_overlay_can_be_configured_to_escape() {
    let yaml = r#"
interface:
  key_bindings:
    activate-overlay: "Ctrl-Space"
    deactivate-overlay: "Esc"
    approve: "Y"
    deny: "N"
    "#;

    let config: TerminaiConfig = serde_yaml::from_str(yaml).unwrap();

    assert!(
      config
        .interface
        .key_bindings
        .deactivate_overlay
        .matches(key!(esc))
    );
  }

  #[test]
  fn test_terminai_config_with_interface() {
    let yaml = r#"
interface:
  chat-position: top
    "#;

    let config: TerminaiConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.interface.chat_position, ChatPosition::Top);

    let yaml2 = r#"
interface:
  chat-position: bottom
    "#;

    let config2: TerminaiConfig = serde_yaml::from_str(yaml2).unwrap();
    assert_eq!(config2.interface.chat_position, ChatPosition::Bottom);
  }

  #[test]
  fn test_agent_config_defaults_to_codex() {
    let yaml = r#"
interface:
  chat-position: bottom
    "#;

    let config: TerminaiConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.agent.effective_kind(), AgentKind::Codex);
    assert_eq!(config.agent.preset.as_deref(), Some("codex"));
    assert_eq!(config.agent.command.as_deref(), None);
  }

  #[test]
  fn test_agent_config_custom_command() {
    let yaml = r#"
agent:
  kind: custom
  command: my-agent
  args:
    - --mcp
    - "{mcp_url}"
    "#;

    let config: TerminaiConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.agent.effective_kind(), AgentKind::Custom);
    assert_eq!(config.agent.command.as_deref(), Some("my-agent"));
    assert_eq!(config.agent.args, vec!["--mcp", "{mcp_url}"]);
  }
}
