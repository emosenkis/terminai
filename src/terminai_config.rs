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

/// How agent-suggested shell input is handled.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(
  Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default,
)]
#[serde(rename_all = "kebab-case")]
pub enum ApprovalMode {
  #[default]
  AlwaysAsk,
  AutoApproval,
}

impl Default for ChatPosition {
  fn default() -> Self {
    Self::Bottom
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
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
  #[serde(
    default = "default_toggle_approval_mode_binding",
    rename = "toggle-approval-mode"
  )]
  pub toggle_approval_mode: OneOrMoreBindings,
  #[serde(default = "default_switch_agent_binding", rename = "switch-agent")]
  pub switch_agent: OneOrMoreBindings,
  #[serde(default = "default_clear_history_binding", rename = "clear-history")]
  pub clear_history: OneOrMoreBindings,
  #[serde(default = "default_control_panel_binding", rename = "control-panel")]
  pub control_panel: OneOrMoreBindings,
}

fn default_toggle_approval_mode_binding() -> OneOrMoreBindings {
  OneOrMoreBindings::Single(key!(f7))
}

fn default_switch_agent_binding() -> OneOrMoreBindings {
  OneOrMoreBindings::Single(key!(f8))
}

fn default_clear_history_binding() -> OneOrMoreBindings {
  OneOrMoreBindings::Single(key!(f9))
}

fn default_control_panel_binding() -> OneOrMoreBindings {
  OneOrMoreBindings::Single(key!(f10))
}

impl Default for KeyBindingsConfig {
  fn default() -> Self {
    Self {
      activate_overlay: OneOrMoreBindings::Single(key!(ctrl - space)),
      deactivate_overlay: OneOrMoreBindings::Single(key!(ctrl - space)),
      approve: OneOrMoreBindings::Single(key!(y)),
      deny: OneOrMoreBindings::Single(key!(n)),
      toggle_approval_mode: default_toggle_approval_mode_binding(),
      switch_agent: default_switch_agent_binding(),
      clear_history: default_clear_history_binding(),
      control_panel: default_control_panel_binding(),
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

/// Default shell for the wrapped terminal. This is a shell selector, not a
/// shell-script launcher; use `terminai -- <command> [args...]` for commands.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schema", schemars(deny_unknown_fields))]
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ShellConfig {
  #[serde(default)]
  pub command: Option<String>,
  #[serde(default)]
  pub args: Vec<String>,
}

/// How matching terminal data is anonymized before being sent to an agent.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(
  Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default,
)]
#[serde(rename_all = "lowercase")]
pub enum PrivacyStrategy {
  #[default]
  Replace,
  Mask,
  Hash,
  Encrypt,
  Redact,
}

/// Pattern-based privacy filtering for terminal text returned to agents.
///
/// `patterns` accepts a Redact entity name (for example `email-address`), a
/// category (`credentials`, `financial`, `identity`, `medical`, `crypto`, or
/// `gitleaks`),
/// or `default`. Prefix a value with `-` to remove an earlier selection.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schema", schemars(deny_unknown_fields))]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct PrivacyConfig {
  #[serde(default = "default_privacy_patterns")]
  pub patterns: Vec<String>,
  #[serde(default)]
  pub strategy: PrivacyStrategy,
}

fn default_privacy_patterns() -> Vec<String> {
  vec!["default".to_string()]
}

impl Default for PrivacyConfig {
  fn default() -> Self {
    Self {
      patterns: default_privacy_patterns(),
      strategy: PrivacyStrategy::default(),
    }
  }
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

/// A command-line argument rendered for an agent process.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schema", schemars(deny_unknown_fields))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum AgentArg {
  /// A static string or Minijinja template that produces one argument.
  Template(String),
  /// A Minijinja expression that produces an array of strings.
  Expression { expr: String },
}

impl From<String> for AgentArg {
  fn from(value: String) -> Self {
    Self::Template(value)
  }
}

impl From<&str> for AgentArg {
  fn from(value: &str) -> Self {
    Self::Template(value.to_string())
  }
}

/// Agent configuration.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schema", schemars(deny_unknown_fields))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct AgentConfig {
  #[serde(default)]
  pub preset: Option<String>,
  #[serde(default)]
  pub kind: Option<AgentKind>,
  #[serde(default)]
  pub command: Option<String>,
  /// CLI arguments passed to the agent.
  ///
  /// Each argument can be a string rendered as a Minijinja template or an
  /// object containing `expr`, whose Minijinja expression must return an array
  /// of strings. Available variables:
  ///
  /// General:
  /// - `{{ cwd }}`: the working directory where the agent starts.
  /// - `{{ context_prompt }}`: the rendered Terminai context prompt for the
  ///   resolved agent config.
  /// - `{{ uses_mcp }}`: whether the resolved agent config enables
  ///   the Terminai MCP server.
  /// - `{{ uses_tool_cli }}`: whether the resolved agent config enables
  ///   Terminai CLI tool instructions.
  ///
  /// MCP:
  /// - `{{ mcp_url }}`: the Terminai MCP server URL.
  /// - `{{ mcp_command }}`: the command to launch MCP.
  /// - `{{ mcp_port }}`: the local port used by the Terminai MCP server.
  ///
  /// The MCP bearer token is available to the agent as the
  /// `TERMINAI_MCP_AUTH_TOKEN` environment variable.
  ///
  /// Tool CLI:
  /// - `{{ tool_command }}`: the command to interact with Terminai-provided
  ///   tools.
  ///
  /// Available filters:
  /// - `{{ value|toml }}`: render `value` as a TOML string.
  /// - `{{ value|json }}`: render `value` as a JSON string.
  ///
  /// For zero or multiple arguments, use an expression object such as
  /// `expr: '["--mcp", mcp_url] if uses_mcp else []'`.
  #[serde(default)]
  pub args: Vec<AgentArg>,
  /// Additional CLI arguments appended after `args`. Supports the same
  /// Minijinja template variables, filters, and expressions documented on
  /// `args`.
  #[serde(default)]
  pub extra_args: Vec<AgentArg>,
  /// Prompt template to render for this agent. Defaults to `default.jinja`,
  /// which is loaded from the Terminai XDG config directory when present and
  /// otherwise falls back to the bundled template. Other names are loaded from
  /// the same directory. `builtin/default.jinja` always names the bundled
  /// template and can be extended by a user-provided default.
  #[serde(default)]
  pub prompt_template: Option<String>,
  /// Whether this agent uses the Terminai MCP server. Defaults to false for
  /// custom agents and inherits from the selected preset when unset.
  #[serde(default)]
  pub uses_mcp: Option<bool>,
  /// Whether this agent uses the Terminai CLI tools. Defaults to true for
  /// custom agents and inherits from the selected preset when unset.
  #[serde(default)]
  pub uses_tool_cli: Option<bool>,
}

impl AgentConfig {
  pub fn claude() -> Self {
    Self {
      preset: Some("claude".to_string()),
      kind: Some(AgentKind::Claude),
      command: None,
      args: Vec::new(),
      extra_args: Vec::new(),
      prompt_template: None,
      uses_mcp: None,
      uses_tool_cli: None,
    }
  }

  pub fn codex() -> Self {
    Self {
      preset: Some("codex".to_string()),
      kind: Some(AgentKind::Codex),
      command: None,
      args: Vec::new(),
      extra_args: Vec::new(),
      prompt_template: None,
      uses_mcp: None,
      uses_tool_cli: None,
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schema", schemars(deny_unknown_fields))]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct AgentPresetConfig {
  #[serde(default)]
  pub extends: Option<String>,
  #[serde(default)]
  pub command: Option<String>,
  /// CLI arguments passed to the agent.
  ///
  /// Each argument can be a string rendered as a Minijinja template or an
  /// object containing `expr`, whose Minijinja expression must return an array
  /// of strings. Available variables:
  ///
  /// General:
  /// - `{{ cwd }}`: the working directory where the agent starts.
  /// - `{{ context_prompt }}`: the rendered Terminai context prompt for the
  ///   resolved agent config.
  /// - `{{ uses_mcp }}`: whether the resolved agent config enables
  ///   the Terminai MCP server.
  /// - `{{ uses_tool_cli }}`: whether the resolved agent config enables
  ///   Terminai CLI tool instructions.
  ///
  /// MCP:
  /// - `{{ mcp_url }}`: the Terminai MCP server URL.
  /// - `{{ mcp_command }}`: the command to launch MCP.
  /// - `{{ mcp_port }}`: the local port used by the Terminai MCP server.
  ///
  /// The MCP bearer token is available to the agent as the
  /// `TERMINAI_MCP_AUTH_TOKEN` environment variable.
  ///
  /// Tool CLI:
  /// - `{{ tool_command }}`: the command to interact with Terminai-provided
  ///   tools.
  ///
  /// Available filters:
  /// - `{{ value|toml }}`: render `value` as a TOML string.
  /// - `{{ value|json }}`: render `value` as a JSON string.
  ///
  /// For zero or multiple arguments, use an expression object such as
  /// `expr: '["--mcp", mcp_url] if uses_mcp else []'`.
  #[serde(default)]
  pub args: Vec<AgentArg>,
  /// Additional CLI arguments appended after `args`. Supports the same
  /// Minijinja template variables, filters, and expressions documented on
  /// `args`.
  #[serde(default)]
  pub extra_args: Vec<AgentArg>,
  /// Prompt template inherited by agents using this preset. Uses the same XDG
  /// lookup and `default.jinja` shadowing behavior as the agent setting.
  #[serde(default)]
  pub prompt_template: Option<String>,
  #[serde(default)]
  pub env: HashMap<String, String>,
  #[serde(default)]
  pub uses_mcp: Option<bool>,
  #[serde(default)]
  pub uses_tool_cli: Option<bool>,
  /// Whether this preset appears in the in-app agent switcher.
  #[serde(default = "default_true")]
  pub show_in_switcher: bool,
}

fn default_true() -> bool {
  true
}

impl Default for AgentPresetConfig {
  fn default() -> Self {
    Self {
      extends: None,
      command: None,
      args: Vec::new(),
      extra_args: Vec::new(),
      prompt_template: None,
      env: HashMap::new(),
      uses_mcp: None,
      uses_tool_cli: None,
      show_in_switcher: true,
    }
  }
}

/// Top-level Terminai configuration loaded from
/// `$XDG_CONFIG_HOME/terminai/terminai.yaml`, falling back to
/// `~/.config/terminai/terminai.yaml` when `XDG_CONFIG_HOME` is unset. On
/// Windows it is `%APPDATA%\\terminai\\terminai.yaml`.
///
/// Default configuration can be installed with `terminai init-config`
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schema", schemars(deny_unknown_fields))]
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct TerminaiConfig {
  /// Startup policy for agent-suggested shell input.
  #[serde(default, rename = "approval-mode")]
  pub approval_mode: ApprovalMode,
  /// Default wrapped shell.
  #[serde(default)]
  pub shell: ShellConfig,
  /// Interface configuration
  #[serde(default)]
  pub interface: InterfaceConfig,
  /// Privacy filtering applied to terminal text returned through MCP.
  #[serde(default)]
  pub privacy: PrivacyConfig,
  /// External CLI agent to run in the AI terminal.
  #[serde(default)]
  pub agent: AgentConfig,
  /// User-defined CLI agent presets. Built-in presets include codex and claude.
  #[serde(default, rename = "agent-presets")]
  pub agent_presets: HashMap<String, AgentPresetConfig>,
}

impl TerminaiConfig {
  pub fn path() -> Result<PathBuf> {
    let expected = crate::paths::config_dir()?.join("terminai.yaml");
    if !expected.exists() {
      anyhow::bail!(
        "Configuration file not found. Expected at: {}",
        expected.display()
      );
    }
    Ok(expected)
  }

  pub fn expected_path() -> Result<PathBuf> {
    Ok(crate::paths::config_dir()?.join("terminai.yaml"))
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
  fn runtime_control_defaults_are_conservative() {
    let config: TerminaiConfig = serde_yaml::from_str("{}").unwrap();

    assert_eq!(config.approval_mode, ApprovalMode::AlwaysAsk);
    assert!(
      config
        .interface
        .key_bindings
        .toggle_approval_mode
        .matches(key!(f7))
    );
    assert!(config.interface.key_bindings.switch_agent.matches(key!(f8)));
    assert!(
      config
        .interface
        .key_bindings
        .clear_history
        .matches(key!(f9))
    );
    assert!(
      config
        .interface
        .key_bindings
        .control_panel
        .matches(key!(f10))
    );
    assert!(AgentPresetConfig::default().show_in_switcher);
  }

  #[test]
  fn auto_approval_and_hidden_presets_deserialize() {
    let config: TerminaiConfig = serde_yaml::from_str(
      r#"
approval-mode: auto-approval
agent-presets:
  hidden:
    command: hidden-agent
    show-in-switcher: false
"#,
    )
    .unwrap();

    assert_eq!(config.approval_mode, ApprovalMode::AutoApproval);
    assert!(!config.agent_presets["hidden"].show_in_switcher);
  }

  #[test]
  fn privacy_config_accepts_categories_removals_and_strategy() {
    let config: TerminaiConfig = serde_yaml::from_str(
      r#"
privacy:
  patterns: [default, -btc-address, credentials]
  strategy: mask
"#,
    )
    .unwrap();

    assert_eq!(
      config.privacy.patterns,
      vec![
        "default".to_string(),
        "-btc-address".to_string(),
        "credentials".to_string(),
      ]
    );
    assert_eq!(config.privacy.strategy, PrivacyStrategy::Mask);
  }

  #[test]
  fn shell_config_deserializes() {
    let config: TerminaiConfig = serde_yaml::from_str(
      "shell:\n  command: pwsh.exe\n  args: [\"-NoLogo\"]\n",
    )
    .unwrap();
    assert_eq!(config.shell.command.as_deref(), Some("pwsh.exe"));
    assert_eq!(config.shell.args, ["-NoLogo"]);
  }

  #[test]
  fn test_agent_config_extra_args_and_user_presets() {
    let yaml = r#"
agent:
  preset: codex
  uses-mcp: true
  uses-tool-cli: false
  extra-args:
    - --model
    - gpt-5.5
agent-presets:
  opencode-fast:
    extends: opencode
    uses-mcp: false
    uses-tool-cli: true
    extra-args:
      - --model
      - github-copilot/gpt-5
    "#;

    let config: TerminaiConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.agent.preset.as_deref(), Some("codex"));
    assert_eq!(config.agent.uses_mcp, Some(true));
    assert_eq!(config.agent.uses_tool_cli, Some(false));
    assert_eq!(
      config.agent.extra_args,
      vec![AgentArg::from("--model"), AgentArg::from("gpt-5.5")]
    );
    let preset = config.agent_presets.get("opencode-fast").unwrap();
    assert_eq!(preset.extends.as_deref(), Some("opencode"));
    assert_eq!(preset.uses_mcp, Some(false));
    assert_eq!(preset.uses_tool_cli, Some(true));
  }

  #[test]
  fn test_agent_args_accept_templates_and_expressions() {
    let yaml = r#"
agent:
  command: my-agent
  prompt-template: custom.jinja
  args:
    - --static
    - "--cwd={{ cwd }}"
    - expr: '["--mcp", mcp_url] if uses_mcp else []'
  extra-args:
    - expr: '["--verbose"]'
    "#;

    let config: TerminaiConfig = serde_yaml::from_str(yaml).unwrap();

    assert_eq!(
      config.agent.prompt_template.as_deref(),
      Some("custom.jinja")
    );
    assert_eq!(
      config.agent.args,
      vec![
        AgentArg::Template("--static".to_string()),
        AgentArg::Template("--cwd={{ cwd }}".to_string()),
        AgentArg::Expression {
          expr: "[\"--mcp\", mcp_url] if uses_mcp else []".to_string(),
        },
      ]
    );
    assert_eq!(
      config.agent.extra_args,
      vec![AgentArg::Expression {
        expr: "[\"--verbose\"]".to_string(),
      }]
    );
  }

  #[test]
  fn test_agent_expression_rejects_unknown_fields() {
    let yaml = r#"
agent:
  args:
    - expr: '["--verbose"]'
      typo: true
    "#;

    serde_yaml::from_str::<TerminaiConfig>(yaml)
      .expect_err("unknown expression fields should fail");
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
    assert_eq!(
      config.agent.args,
      vec![AgentArg::from("--mcp"), AgentArg::from("{mcp_url}")]
    );
  }
}
