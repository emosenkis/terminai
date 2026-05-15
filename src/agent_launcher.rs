use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;

use crate::terminai_config::{AgentConfig, AgentKind};

pub const TERMINAI_AGENT_PROMPT: &str = r#"You are running inside Termin.AI, a terminal wrapper that is displaying your CLI as a secondary terminal.

Important Termin.AI rules:
- You are NOT talking directly to the user's wrapped shell. Your own terminal is only the AI CLI terminal.
- To understand the user's shell, use the Termin.AI MCP tool read_terminal before answering terminal-state questions.
- To inspect shell metadata, use get_terminal_context.
- To help the user run something in the wrapped shell, call suggest_input with the exact input and a short explanation.
- Do not claim you ran a command in the wrapped shell unless Termin.AI confirms the user approved it.
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
  context: &AgentLaunchContext,
) -> Result<AgentLaunchPlan> {
  let kind = config.effective_kind();
  let mut env = HashMap::new();
  env.insert("TERMINAI_MCP_URL".to_string(), context.mcp_url.clone());
  env.insert(
    "TERMINAI_CONTEXT_PROMPT".to_string(),
    context.context_prompt.clone(),
  );

  let (command, args) = match kind {
    AgentKind::Claude => claude_args(config, context),
    AgentKind::Codex => codex_args(config, context),
    AgentKind::Custom => custom_args(config, context),
  };

  Ok(AgentLaunchPlan {
    command,
    args,
    env,
    cwd: context.cwd.clone(),
  })
}

fn claude_args(
  config: &AgentConfig,
  context: &AgentLaunchContext,
) -> (String, Vec<String>) {
  let command = config
    .command
    .clone()
    .unwrap_or_else(|| "claude".to_string());
  let mut args = config.args.clone();
  if args.is_empty() {
    args.extend([
      "--append-system-prompt".to_string(),
      context.context_prompt.clone(),
      "--mcp-config".to_string(),
      claude_mcp_config(&context.mcp_url),
      "--strict-mcp-config".to_string(),
      "--permission-mode".to_string(),
      "default".to_string(),
    ]);
  }
  (command, expand_args(args, context))
}

fn codex_args(
  config: &AgentConfig,
  context: &AgentLaunchContext,
) -> (String, Vec<String>) {
  let command = config
    .command
    .clone()
    .unwrap_or_else(|| "codex".to_string());
  let mut args = config.args.clone();
  if args.is_empty() {
    args.extend([
      "--cd".to_string(),
      "{cwd}".to_string(),
      "--sandbox".to_string(),
      "workspace-write".to_string(),
      "--ask-for-approval".to_string(),
      "on-request".to_string(),
      "-c".to_string(),
      format!("mcp_servers.terminai.url={:?}", context.mcp_url),
      "{context_prompt}".to_string(),
    ]);
  }
  (command, expand_args(args, context))
}

fn custom_args(
  config: &AgentConfig,
  context: &AgentLaunchContext,
) -> (String, Vec<String>) {
  let command = config
    .command
    .clone()
    .unwrap_or_else(|| "codex".to_string());
  (command, expand_args(config.args.clone(), context))
}

fn expand_args(args: Vec<String>, context: &AgentLaunchContext) -> Vec<String> {
  let cwd = context.cwd.display().to_string();
  args
    .into_iter()
    .map(|arg| {
      arg
        .replace("{cwd}", &cwd)
        .replace("{mcp_url}", &context.mcp_url)
        .replace("{context_prompt}", &context.context_prompt)
    })
    .collect()
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
    let plan = build_launch_plan(&config, &context()).unwrap();

    assert_eq!(plan.command, "codex");
    assert!(plan.args.contains(&"--cd".to_string()));
    assert!(plan.args.contains(&"/tmp/project".to_string()));
    assert!(plan.args.contains(&"--sandbox".to_string()));
    assert!(plan.args.contains(&"workspace-write".to_string()));
    assert!(plan.args.iter().any(|arg| arg.contains("mcp_servers")));
    let prompt = plan.args.last().unwrap();
    assert!(prompt.contains("read_terminal before answering"));
    assert!(prompt.contains("suggest_input with the exact input"));
    assert!(prompt.contains("Do not claim you ran a command"));
  }

  #[test]
  fn claude_plan_injects_mcp_config_and_prompt() {
    let config = AgentConfig::claude();
    let plan = build_launch_plan(&config, &context()).unwrap();

    assert_eq!(plan.command, "claude");
    assert!(plan.args.contains(&"--append-system-prompt".to_string()));
    assert!(plan.args.contains(&"--mcp-config".to_string()));
    assert!(plan.args.iter().any(|arg| arg.contains("terminai")));
    assert!(plan.args.iter().any(|arg| arg.contains("127.0.0.1")));
  }

  #[test]
  fn custom_plan_expands_templates() {
    let config = AgentConfig {
      kind: Some(AgentKind::Custom),
      command: Some("my-agent".to_string()),
      args: vec!["--url={mcp_url}".to_string(), "--cwd={cwd}".to_string()],
      initial_prompt: None,
    };
    let plan = build_launch_plan(&config, &context()).unwrap();

    assert_eq!(plan.command, "my-agent");
    assert_eq!(plan.args[0], "--url=http://127.0.0.1:3456/mcp");
    assert_eq!(plan.args[1], "--cwd=/tmp/project");
  }
}
