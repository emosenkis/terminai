use std::collections::HashMap;

use anyhow::Result;
use crokey::KeyCombination;
use crokey::key;
use serde::{Deserialize, Serialize};

/// Position of the AI chat overlay
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

impl OneOrMoreBindings {
  pub fn matches(&self, key_combo: KeyCombination) -> bool {
    match self {
      OneOrMoreBindings::Single(key) => *key == key_combo,
      OneOrMoreBindings::Multiple(keys) => keys.contains(&key_combo),
    }
  }
}

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
      deactivate_overlay: OneOrMoreBindings::Multiple(vec![
        key!(ctrl - space),
        key!(esc),
      ]),
      approve: OneOrMoreBindings::Single(key!(y)),
      deny: OneOrMoreBindings::Single(key!(n)),
    }
  }
}

/// Interface configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct InterfaceConfig {
  /// Position of the AI chat overlay (default: bottom)
  #[serde(default, rename = "chat-position")]
  pub chat_position: ChatPosition,
  /// Key bindings
  #[serde(default)]
  pub key_bindings: KeyBindingsConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentKind {
  Claude,
  Codex,
  Custom,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentConfig {
  #[serde(default)]
  pub preset: Option<String>,
  #[serde(default)]
  pub kind: Option<AgentKind>,
  #[serde(default)]
  pub command: Option<String>,
  #[serde(default)]
  pub args: Vec<String>,
  #[serde(default, rename = "extra-args")]
  pub extra_args: Vec<String>,
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

/// Configuration for a specific AI model
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelConfig {
  /// Display name for the model (e.g., "Claude Sonnet 4.5")
  pub name: String,
  /// Model identifier (e.g., "claude-sonnet-4-5")
  pub model: String,
}

/// Configuration for an AI provider
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderConfig {
  /// Provider name (anthropic, openai, gemini, openrouter, etc.)
  pub name: String,
  /// Optional display name (defaults to name)
  #[serde(default)]
  pub display_name: Option<String>,
  /// Optional environment variable name for API key
  #[serde(default)]
  pub api_key_env: Option<String>,
  /// List of available models
  pub models: Vec<ModelConfig>,
}

impl ProviderConfig {
  /// Get the effective display name
  pub fn effective_display_name(&self) -> &str {
    self.display_name.as_deref().unwrap_or(&self.name)
  }

  /// Get the environment variable name to use for the API key
  /// Uses api_key_env if specified, otherwise falls back to provider default
  pub fn effective_api_key_env(&self) -> Option<String> {
    if let Some(ref env_var) = self.api_key_env {
      return Some(env_var.clone());
    }

    match self.name.as_str() {
      "anthropic" => Some("ANTHROPIC_API_KEY".to_string()),
      "openai" => Some("OPENAI_API_KEY".to_string()),
      "gemini" => Some("GEMINI_API_KEY".to_string()),
      "openrouter" => Some("OPENROUTER_API_KEY".to_string()),
      "ollama" => None,
      _ => None,
    }
  }
}

/// Top-level Termin.AI configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct TerminAIConfig {
  /// Interface configuration
  #[serde(default)]
  pub interface: InterfaceConfig,
  /// List of AI providers with their models
  #[serde(default)]
  pub providers: Vec<ProviderConfig>,
  /// Default model in format "provider/model" (e.g., "anthropic/claude-sonnet-4-5")
  #[serde(default)]
  pub default_model: String,
  /// External CLI agent to run in the AI terminal.
  #[serde(default)]
  pub agent: AgentConfig,
  /// User-defined CLI agent presets. Built-in presets include codex and claude.
  #[serde(default, rename = "agent-presets")]
  pub agent_presets: HashMap<String, AgentPresetConfig>,
}

impl TerminAIConfig {
  /// Load configuration from XDG config directory (~/.config/terminai/terminai.yaml)
  pub fn load() -> Result<Self> {
    let config_dir = xdg::BaseDirectories::with_prefix("terminai");
    let config_path =
      config_dir
        .find_config_file("terminai.yaml")
        .ok_or_else(|| {
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
        })?;

    log::info!("Loading configuration from: {}", config_path.display());
    let config_content = std::fs::read_to_string(&config_path)?;
    // TODO: Switch to HJSON? It's simpler and safer than YAML
    let config: TerminAIConfig = serde_yaml::from_str(&config_content)?;

    log::info!(
      "Configuration loaded: {} providers, default model: {}",
      config.providers.len(),
      config.default_model
    );

    Ok(config)
  }

  /// Parse the default model string into (provider, model) tuple
  pub fn parse_default_model(&self) -> Result<(&str, &str)> {
    let parts: Vec<&str> = self.default_model.split('/').collect();
    if parts.len() != 2 {
      anyhow::bail!(
        "Invalid default_model format '{}'. Expected 'provider/model'",
        self.default_model
      );
    }
    Ok((parts[0], parts[1]))
  }

  /// Find a provider by name
  pub fn find_provider(&self, name: &str) -> Option<&ProviderConfig> {
    self.providers.iter().find(|p| p.name == name)
  }

  /// Get the default provider and model
  pub fn get_default_provider_and_model(
    &self,
  ) -> Result<(&ProviderConfig, &ModelConfig)> {
    let (provider_name, model_id) = self.parse_default_model()?;

    let provider = self.find_provider(provider_name).ok_or_else(|| {
      anyhow::anyhow!(
        "Default provider '{}' not found in providers list",
        provider_name
      )
    })?;

    let model = provider
      .models
      .iter()
      .find(|m| m.model == model_id)
      .ok_or_else(|| {
        anyhow::anyhow!(
          "Default model '{}' not found in provider '{}' models list",
          model_id,
          provider_name
        )
      })?;

    Ok((provider, model))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_terminai_config_yaml_deserialize() {
    let yaml = r#"
providers:
  - name: anthropic
    display_name: Anthropic
    api_key_env: ANTHROPIC_API_KEY
    models:
      - name: "Claude Sonnet 4.5"
        model: claude-sonnet-4-5
      - name: "Claude Haiku 4.5"
        model: claude-haiku-4-5
  - name: openai
    models:
      - name: "GPT 5.1"
        model: gpt-5.1
default_model: anthropic/claude-sonnet-4-5
    "#;

    let config: TerminAIConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.providers.len(), 2);
    assert_eq!(config.providers[0].name, "anthropic");
    assert_eq!(
      config.providers[0].display_name,
      Some("Anthropic".to_string())
    );
    assert_eq!(config.providers[0].models.len(), 2);
    assert_eq!(config.providers[0].models[0].name, "Claude Sonnet 4.5");
    assert_eq!(config.providers[0].models[0].model, "claude-sonnet-4-5");

    assert_eq!(config.providers[1].name, "openai");
    assert_eq!(config.providers[1].display_name, None);
    assert_eq!(config.providers[1].models.len(), 1);

    assert_eq!(config.default_model, "anthropic/claude-sonnet-4-5");
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
providers: []
default_model: ""
    "#;

    let config: TerminAIConfig = serde_yaml::from_str(yaml).unwrap();
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
  fn test_terminai_config_with_interface() {
    let yaml = r#"
interface:
  chat-position: top
providers:
  - name: anthropic
    models:
      - name: "Claude Sonnet"
        model: claude-sonnet-4-5
default_model: anthropic/claude-sonnet-4-5
    "#;

    let config: TerminAIConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.interface.chat_position, ChatPosition::Top);

    let yaml2 = r#"
interface:
  chat-position: bottom
providers:
  - name: anthropic
    models:
      - name: "Claude Sonnet"
        model: claude-sonnet-4-5
default_model: anthropic/claude-sonnet-4-5
    "#;

    let config2: TerminAIConfig = serde_yaml::from_str(yaml2).unwrap();
    assert_eq!(config2.interface.chat_position, ChatPosition::Bottom);
  }

  #[test]
  fn test_terminai_config_parse_default_model() {
    let yaml = r#"
providers:
  - name: anthropic
    models:
      - name: "Claude Sonnet"
        model: claude-sonnet-4-5
default_model: anthropic/claude-sonnet-4-5
    "#;

    let config: TerminAIConfig = serde_yaml::from_str(yaml).unwrap();
    let (provider, model) = config.parse_default_model().unwrap();
    assert_eq!(provider, "anthropic");
    assert_eq!(model, "claude-sonnet-4-5");
  }

  #[test]
  fn test_terminai_config_get_default_provider_and_model() {
    let yaml = r#"
providers:
  - name: anthropic
    models:
      - name: "Claude Sonnet 4.5"
        model: claude-sonnet-4-5
      - name: "Claude Haiku 4.5"
        model: claude-haiku-4-5
  - name: openai
    models:
      - name: "GPT 5.1"
        model: gpt-5.1
default_model: anthropic/claude-haiku-4-5
    "#;

    let config: TerminAIConfig = serde_yaml::from_str(yaml).unwrap();
    let (provider, model) = config.get_default_provider_and_model().unwrap();
    assert_eq!(provider.name, "anthropic");
    assert_eq!(model.name, "Claude Haiku 4.5");
    assert_eq!(model.model, "claude-haiku-4-5");
  }

  #[test]
  fn test_terminai_config_find_provider() {
    let yaml = r#"
providers:
  - name: anthropic
    models:
      - name: "Claude Sonnet"
        model: claude-sonnet-4-5
  - name: openai
    models:
      - name: "GPT 5.1"
        model: gpt-5.1
default_model: anthropic/claude-sonnet-4-5
    "#;

    let config: TerminAIConfig = serde_yaml::from_str(yaml).unwrap();
    assert!(config.find_provider("anthropic").is_some());
    assert!(config.find_provider("openai").is_some());
    assert!(config.find_provider("gemini").is_none());
  }

  #[test]
  fn test_provider_config_effective_display_name() {
    let provider = ProviderConfig {
      name: "anthropic".to_string(),
      display_name: Some("Anthropic AI".to_string()),
      api_key_env: None,
      models: vec![],
    };
    assert_eq!(provider.effective_display_name(), "Anthropic AI");

    let provider2 = ProviderConfig {
      name: "openai".to_string(),
      display_name: None,
      api_key_env: None,
      models: vec![],
    };
    assert_eq!(provider2.effective_display_name(), "openai");
  }

  #[test]
  fn test_agent_config_defaults_to_codex() {
    let yaml = r#"
interface:
  chat-position: bottom
    "#;

    let config: TerminAIConfig = serde_yaml::from_str(yaml).unwrap();
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

    let config: TerminAIConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.agent.effective_kind(), AgentKind::Custom);
    assert_eq!(config.agent.command.as_deref(), Some("my-agent"));
    assert_eq!(config.agent.args, vec!["--mcp", "{mcp_url}"]);
  }

  #[test]
  fn test_provider_config_effective_api_key_env() {
    let provider = ProviderConfig {
      name: "anthropic".to_string(),
      display_name: None,
      api_key_env: Some("MY_CUSTOM_KEY".to_string()),
      models: vec![],
    };
    assert_eq!(
      provider.effective_api_key_env(),
      Some("MY_CUSTOM_KEY".to_string())
    );

    let provider2 = ProviderConfig {
      name: "anthropic".to_string(),
      display_name: None,
      api_key_env: None,
      models: vec![],
    };
    assert_eq!(
      provider2.effective_api_key_env(),
      Some("ANTHROPIC_API_KEY".to_string())
    );
  }
}
