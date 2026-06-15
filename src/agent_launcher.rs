use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{Result, bail};

use crate::terminai_config::{AgentConfig, AgentKind, AgentPresetConfig};

pub const TERMINAI_AGENT_PROMPT: &str = r#"You are running inside Termin.AI, a terminal wrapper that is displaying your CLI as a secondary terminal.

Important Termin.AI rules:
- While you can perform general coding and other tasks, your base assumption where reasonable is that the user wants help with
  whatever they're doing in their own terminal.
- You are NOT typing directly into the user's terminal. Your own terminal is only the AI CLI terminal.
- When you refer to terminal state, say "your terminal" to the user. Do not call it "the wrapped terminal" or "the host terminal".
- Before starting to handle any user message, always call the Termin.AI MCP tool check_for_updates.
- Do not tell the user that you are calling check_for_updates. Silently take into account any updates it returns.
- To understand the user's terminal, use the Termin.AI MCP tool read_terminal before answering terminal-state questions.
- To inspect shell metadata, use get_terminal_context.
- check_for_updates reports user-terminal changes such as cwd changes since your last update check.
- To help the user run something in their terminal, call suggest_input with the exact input and a short explanation.
- Do not claim you ran a command in the user's terminal unless Termin.AI confirms the user approved it.
- Use escape sequences in suggestions: \r for Enter, \u0003 for Ctrl-C, \u001b for Escape.
"#;

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
      context_prompt: TERMINAI_AGENT_PROMPT.to_string(),
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

fn builtin_preset(name: &str) -> Option<AgentPresetConfig> {
  match name {
    "claude" => Some(AgentPresetConfig {
      command: Some("claude".to_string()),
      args: vec![
      "--append-system-prompt".to_string(),
      "{context_prompt}".to_string(),
      "--mcp-config".to_string(),
      "{claude_mcp_config}".to_string(),
      "--strict-mcp-config".to_string(),
      "--permission-mode".to_string(),
      "default".to_string(),
      ],
      ..Default::default()
    }),
    "codex" => Some(AgentPresetConfig {
      command: Some("codex".to_string()),
      args: vec![
      "--cd".to_string(),
      "{cwd}".to_string(),
      "--sandbox".to_string(),
      "workspace-write".to_string(),
      "--ask-for-approval".to_string(),
      "on-request".to_string(),
      "--no-alt-screen".to_string(),
      "-c".to_string(),
      "developer_instructions={context_prompt_toml}".to_string(),
      "-c".to_string(),
      "mcp_servers.terminai.url={mcp_url_toml}".to_string(),
      "-c".to_string(),
      "mcp_servers.terminai.enabled_tools=[\"check_for_updates\",\"read_terminal\",\"get_terminal_context\",\"suggest_input\",\"get_suggestion_status\"]".to_string(),
      "-c".to_string(),
      "mcp_servers.terminai.default_tools_approval_mode=\"approve\"".to_string(),
      "-c".to_string(),
      "mcp_servers.terminai.tools.check_for_updates.approval_mode=\"approve\"".to_string(),
      "-c".to_string(),
      "mcp_servers.terminai.tools.read_terminal.approval_mode=\"approve\"".to_string(),
      "-c".to_string(),
      "mcp_servers.terminai.tools.get_terminal_context.approval_mode=\"approve\"".to_string(),
      "-c".to_string(),
      "mcp_servers.terminai.tools.suggest_input.approval_mode=\"approve\"".to_string(),
      "-c".to_string(),
      "mcp_servers.terminai.tools.get_suggestion_status.approval_mode=\"approve\"".to_string(),
      ],
      ..Default::default()
    }),
    // These intentionally provide only generic prompt/context plumbing. Users
    // can extend them with MCP-specific flags as each CLI evolves.
    "gemini" => Some(AgentPresetConfig {
      command: Some("gemini".to_string()),
      args: vec!["{context_prompt}".to_string()],
      ..Default::default()
    }),
    "opencode" => Some(AgentPresetConfig {
      command: Some("opencode".to_string()),
      args: vec!["{context_prompt}".to_string()],
      ..Default::default()
    }),
    _ => None,
  }
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
      Some("gemini") => Some("gemini".to_string()),
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

  let preset = user_presets
    .get(name)
    .cloned()
    .or_else(|| builtin_preset(name))
    .ok_or_else(|| anyhow::anyhow!("unknown agent preset '{name}'"))?;

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
        .replace("{context_prompt}", &context.context_prompt)
        .replace("{claude_mcp_config}", &claude_mcp_config(&context.mcp_url)),
    );
  }

  expanded
}

fn toml_string(value: &str) -> String {
  format!("{value:?}")
}

fn claude_mcp_config(mcp_url: &str) -> String {
  serde_json::json!({
    "mcpServers": {
      "terminai": {
        "type": "http",
        "url": mcp_url
      }
    }
  })
  .to_string()
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
    assert!(!plan.args.iter().any(|arg| arg == &TERMINAI_AGENT_PROMPT));
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
