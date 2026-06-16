use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::terminai_config::{AgentConfig, AgentKind, AgentPresetConfig};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentLaunchPlan {
  pub command: String,
  pub args: Vec<String>,
  pub env: HashMap<String, String>,
  pub cwd: PathBuf,
}

#[derive(Debug, Clone)]
pub struct AgentLaunchContext {
  pub cwd: PathBuf,
  pub mcp_url: String,
  pub context_prompt: String,
}

impl AgentLaunchContext {
  pub fn new(cwd: PathBuf, mcp_url: String) -> Self {
    Self {
      cwd,
      mcp_url,
      context_prompt: builtin_context_prompt(),
    }
  }
}

pub fn build_launch_plan(
  config: &AgentConfig,
  user_presets: &HashMap<String, AgentPresetConfig>,
  context: &AgentLaunchContext,
) -> Result<AgentLaunchPlan> {
  let mut env = HashMap::new();
  env.insert("TERMINAI_MCP_URL".to_string(), context.mcp_url.clone());
  env.insert(
    "TERMINAI_CONTEXT_PROMPT".to_string(),
    context.context_prompt.clone(),
  );

  let resolved = resolve_agent_config(config, user_presets)?;
  env.extend(resolved.env);
  let command = resolved.command;
  let args = expand_args(resolved.args, context);

  Ok(AgentLaunchPlan {
    command,
    args,
    env,
    cwd: context.cwd.clone(),
  })
}

#[derive(Debug, Clone)]
struct ResolvedAgentConfig {
  command: String,
  args: Vec<String>,
  env: HashMap<String, String>,
}

const BUILTIN_AGENT_PRESET_CONFIGS: &[(&str, &str)] = &[
  ("config/codex.yaml", include_str!("../config/codex.yaml")),
  ("config/claude.yaml", include_str!("../config/claude.yaml")),
  (
    "config/opencode.yaml",
    include_str!("../config/opencode.yaml"),
  ),
  (
    "config/general.yaml",
    include_str!("../config/general.yaml"),
  ),
];

#[derive(Debug, Default)]
struct BuiltinAgentConfig {
  context_prompt: Option<String>,
  presets: HashMap<String, AgentPresetConfig>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct BuiltinAgentConfigFile {
  #[serde(default)]
  context_prompt: Option<String>,
  #[serde(flatten)]
  presets: HashMap<String, AgentPresetConfig>,
}

fn builtin_agent_config() -> Result<BuiltinAgentConfig> {
  let mut config = BuiltinAgentConfig::default();

  for (path, contents) in BUILTIN_AGENT_PRESET_CONFIGS {
    let file_config: BuiltinAgentConfigFile = serde_yaml::from_str(contents)
      .with_context(|| format!("failed to parse bundled {path}"))?;

    if let Some(context_prompt) = file_config.context_prompt {
      if config.context_prompt.replace(context_prompt).is_some() {
        bail!("bundled context-prompt is defined more than once");
      }
    }

    for (name, preset) in file_config.presets {
      if config.presets.insert(name.clone(), preset).is_some() {
        bail!("bundled agent preset '{name}' is defined more than once");
      }
    }
  }

  Ok(config)
}

fn builtin_context_prompt() -> String {
  builtin_agent_config()
    .expect("bundled agent YAML must parse")
    .context_prompt
    .expect("bundled agent YAML must define context-prompt")
}

fn builtin_agent_presets() -> Result<HashMap<String, AgentPresetConfig>> {
  Ok(builtin_agent_config()?.presets)
}

fn builtin_preset(name: &str) -> Result<Option<AgentPresetConfig>> {
  Ok(builtin_agent_presets()?.remove(name))
}

fn resolve_agent_config(
  config: &AgentConfig,
  user_presets: &HashMap<String, AgentPresetConfig>,
) -> Result<ResolvedAgentConfig> {
  let preset_name = config.preset.clone().or_else(|| match config.kind {
    Some(AgentKind::Claude) => Some("claude".to_string()),
    Some(AgentKind::Codex) => Some("codex".to_string()),
    Some(AgentKind::Custom) => None,
    None => match config.command.as_deref() {
      Some("claude") => Some("claude".to_string()),
      Some("codex") | None => Some("codex".to_string()),
      Some("opencode") => Some("opencode".to_string()),
      Some(_) => None,
    },
  });

  let mut resolved = if let Some(name) = preset_name {
    resolve_preset(&name, user_presets, &mut HashSet::new())?
  } else {
    ResolvedAgentConfig {
      command: config.command.clone().ok_or_else(|| {
        anyhow::anyhow!("custom agent config requires command")
      })?,
      args: Vec::new(),
      env: HashMap::new(),
    }
  };

  if let Some(command) = &config.command {
    resolved.command = command.clone();
  }
  if !config.args.is_empty() {
    resolved.args = config.args.clone();
  }
  resolved.args.extend(config.extra_args.clone());

  Ok(resolved)
}

fn resolve_preset(
  name: &str,
  user_presets: &HashMap<String, AgentPresetConfig>,
  seen: &mut HashSet<String>,
) -> Result<ResolvedAgentConfig> {
  if !seen.insert(name.to_string()) {
    bail!("agent preset '{name}' extends itself recursively");
  }

  let preset = if let Some(preset) = user_presets.get(name).cloned() {
    preset
  } else {
    builtin_preset(name)?
      .ok_or_else(|| anyhow::anyhow!("unknown agent preset '{name}'"))?
  };

  let mut resolved = if let Some(parent) = &preset.extends {
    resolve_preset(parent, user_presets, seen)?
  } else {
    ResolvedAgentConfig {
      command: String::new(),
      args: Vec::new(),
      env: HashMap::new(),
    }
  };

  if let Some(command) = preset.command {
    resolved.command = command;
  }
  if !preset.args.is_empty() {
    resolved.args = preset.args;
  }
  resolved.args.extend(preset.extra_args);
  resolved.env.extend(preset.env);

  if resolved.command.is_empty() {
    bail!("agent preset '{name}' does not define a command");
  }

  seen.remove(name);
  Ok(resolved)
}

fn expand_args(args: Vec<String>, context: &AgentLaunchContext) -> Vec<String> {
  let cwd = context.cwd.display().to_string();
  let mut expanded = Vec::new();

  for arg in args {
    expanded.push(
      arg
        .replace("{cwd}", &cwd)
        .replace("{mcp_url}", &context.mcp_url)
        .replace("{mcp_url_toml}", &toml_string(&context.mcp_url))
        .replace(
          "{context_prompt_toml}",
          &toml_string(&context.context_prompt),
        )
        .replace("{context_prompt}", &context.context_prompt),
    );
  }

  expanded
}

fn toml_string(value: &str) -> String {
  format!("{value:?}")
}

#[cfg(test)]
mod tests {
  use super::*;

  fn context() -> AgentLaunchContext {
    AgentLaunchContext::new(
      PathBuf::from("/tmp/project"),
      "http://127.0.0.1:3456/mcp".to_string(),
    )
  }

  #[test]
  fn bundled_agent_presets_are_parseable_reference_configs() {
    let presets = builtin_agent_presets().unwrap();

    assert!(presets.contains_key("codex"));
    assert!(presets.contains_key("claude"));
    assert!(presets.contains_key("opencode"));
    assert!(!presets.contains_key("deprecated-agent"));
    assert_eq!(
      presets.get("codex").unwrap().command.as_deref(),
      Some("codex")
    );
    assert!(
      builtin_agent_config()
        .unwrap()
        .context_prompt
        .unwrap()
        .contains("check_for_updates")
    );
  }

  #[test]
  fn codex_plan_passes_extremely_clear_e2e_instructions() {
    let config = AgentConfig::codex();
    let plan = build_launch_plan(&config, &HashMap::new(), &context()).unwrap();

    assert_eq!(plan.command, "codex");
    assert!(plan.args.contains(&"--cd".to_string()));
    assert!(plan.args.contains(&"/tmp/project".to_string()));
    assert!(plan.args.contains(&"--sandbox".to_string()));
    assert!(plan.args.contains(&"workspace-write".to_string()));
    assert!(plan.args.contains(&"--no-alt-screen".to_string()));
    assert!(plan.args.iter().any(|arg| arg.contains("mcp_servers")));
    assert!(
      plan
        .args
        .iter()
        .any(|arg| arg.contains("check_for_updates"))
    );
    let developer_instructions = plan
      .args
      .iter()
      .find(|arg| arg.contains("developer_instructions"))
      .unwrap();
    assert!(developer_instructions.contains("always call"));
    assert!(developer_instructions.contains("check_for_updates"));
    assert!(developer_instructions.contains("Do not tell the user"));
    assert!(developer_instructions.contains("read_terminal before answering"));
    assert!(developer_instructions.contains("your terminal"));
    assert!(
      developer_instructions.contains("suggest_input with the exact input")
    );
    assert!(developer_instructions.contains("Do not claim you ran a command"));
    let context = context();
    assert!(!plan.args.iter().any(|arg| arg == &context.context_prompt));
    assert!(
      plan
        .args
        .iter()
        .any(|arg| arg.contains("default_tools_approval_mode=\"approve\""))
    );
    assert!(
      plan
        .args
        .iter()
        .any(|arg| arg.contains("approval_mode=\"approve\""))
    );
  }

  #[test]
  fn claude_plan_injects_mcp_config_and_prompt() {
    let config = AgentConfig::claude();
    let plan = build_launch_plan(&config, &HashMap::new(), &context()).unwrap();

    assert_eq!(plan.command, "claude");
    assert!(plan.args.contains(&"--append-system-prompt".to_string()));
    assert!(plan.args.contains(&"--mcp-config".to_string()));
    assert!(plan.args.iter().any(|arg| arg.contains("terminai")));
    assert!(plan.args.iter().any(|arg| arg.contains("127.0.0.1")));
  }

  #[test]
  fn custom_plan_expands_templates() {
    let config = AgentConfig {
      preset: None,
      kind: Some(AgentKind::Custom),
      command: Some("my-agent".to_string()),
      args: vec!["--url={mcp_url}".to_string(), "--cwd={cwd}".to_string()],
      extra_args: Vec::new(),
      initial_prompt: None,
    };
    let plan = build_launch_plan(&config, &HashMap::new(), &context()).unwrap();

    assert_eq!(plan.command, "my-agent");
    assert_eq!(plan.args[0], "--url=http://127.0.0.1:3456/mcp");
    assert_eq!(plan.args[1], "--cwd=/tmp/project");
  }

  #[test]
  fn user_preset_can_extend_builtin_and_append_flags() {
    let mut presets = HashMap::new();
    presets.insert(
      "codex-fast".to_string(),
      AgentPresetConfig {
        extends: Some("codex".to_string()),
        extra_args: vec!["--model".to_string(), "gpt-5.5".to_string()],
        ..Default::default()
      },
    );
    let config = AgentConfig {
      preset: Some("codex-fast".to_string()),
      extra_args: vec!["--search".to_string()],
      ..Default::default()
    };

    let plan = build_launch_plan(&config, &presets, &context()).unwrap();

    assert_eq!(plan.command, "codex");
    assert!(plan.args.contains(&"--no-alt-screen".to_string()));
    assert_eq!(
      &plan.args[plan.args.len() - 3..],
      ["--model", "gpt-5.5", "--search"]
    );
  }
}
