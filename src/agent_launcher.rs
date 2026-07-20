use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context as AnyhowContext, Result, bail};
use minijinja::{
  AutoEscape, Environment, Error as MinijinjaError,
  ErrorKind as MinijinjaErrorKind, UndefinedBehavior,
};
use serde::{Deserialize, Serialize};

use crate::terminai_config::{
  AgentArg, AgentConfig, AgentKind, AgentPresetConfig,
};

const DEFAULT_PROMPT_TEMPLATE: &str = "default.jinja";
const BUILTIN_DEFAULT_PROMPT_TEMPLATE: &str = "builtin/default.jinja";
const BUILTIN_DEFAULT_PROMPT: &str = include_str!("../config/default.jinja");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentLaunchPlan {
  pub command: String,
  pub args: Vec<String>,
  pub env: HashMap<String, String>,
  pub cwd: PathBuf,
  pub metadata: AgentLaunchMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AgentLaunchMetadata {
  pub mcp_url: String,
  pub mcp_auth_token: String,
  pub terminai_binary_path: String,
  pub terminai_tool_command: String,
  pub terminai_mcp_command: String,
  pub terminai_mcp_port: String,
}

#[derive(Debug, Clone)]
pub struct AgentLaunchContext {
  pub cwd: PathBuf,
  pub mcp_url: String,
  pub mcp_auth_token: String,
  pub terminai_binary_path: String,
  pub terminai_tool_command: String,
  pub terminai_mcp_command: String,
  pub terminai_mcp_port: String,
  pub config_dir: Option<PathBuf>,
}

impl AgentLaunchContext {
  pub fn new(
    cwd: PathBuf,
    mcp_url: String,
    mcp_auth_token: String,
    terminai_binary_path: String,
    terminai_mcp_port: String,
  ) -> Self {
    let terminai_tool_command = format!("{terminai_binary_path} tool");
    let terminai_mcp_command = format!("{terminai_binary_path} _mcp");
    Self {
      cwd,
      mcp_url,
      mcp_auth_token,
      terminai_binary_path,
      terminai_tool_command,
      terminai_mcp_command,
      terminai_mcp_port,
      config_dir: crate::paths::config_dir().ok(),
    }
  }
}

pub fn build_launch_plan(
  config: &AgentConfig,
  user_presets: &HashMap<String, AgentPresetConfig>,
  context: &AgentLaunchContext,
) -> Result<AgentLaunchPlan> {
  let resolved = resolve_agent_config(config, user_presets)?;
  let mut env = resolved.env;
  env.insert(
    "TERMINAI_MCP_AUTH_TOKEN".to_string(),
    context.mcp_auth_token.clone(),
  );
  env.insert(
    "TERMINAI_MCP_PORT".to_string(),
    context.terminai_mcp_port.clone(),
  );

  let command = resolved.command;
  let environment = launch_template_environment(context.config_dir.clone());
  let rendered_context_prompt = render_context_prompt(
    &environment,
    context,
    &resolved.prompt_template,
    resolved.uses_mcp,
    resolved.uses_tool_cli,
  )?;
  let args = expand_args(
    &environment,
    resolved.args,
    context,
    &rendered_context_prompt,
    resolved.uses_mcp,
    resolved.uses_tool_cli,
  )?;
  let metadata = AgentLaunchMetadata {
    mcp_url: context.mcp_url.clone(),
    mcp_auth_token: context.mcp_auth_token.clone(),
    terminai_binary_path: context.terminai_binary_path.clone(),
    terminai_tool_command: context.terminai_tool_command.clone(),
    terminai_mcp_command: context.terminai_mcp_command.clone(),
    terminai_mcp_port: context.terminai_mcp_port.clone(),
  };

  Ok(AgentLaunchPlan {
    command,
    args,
    env,
    cwd: context.cwd.clone(),
    metadata,
  })
}

#[derive(Debug, Clone)]
struct ResolvedAgentConfig {
  command: String,
  args: Vec<AgentArg>,
  env: HashMap<String, String>,
  uses_mcp: bool,
  uses_tool_cli: bool,
  prompt_template: String,
}

const BUILTIN_AGENT_PRESET_CONFIGS: &[(&str, &str)] = &[
  ("config/codex.yaml", include_str!("../config/codex.yaml")),
  ("config/claude.yaml", include_str!("../config/claude.yaml")),
  (
    "config/opencode.yaml",
    include_str!("../config/opencode.yaml"),
  ),
];

#[derive(Debug, Default)]
struct BuiltinAgentConfig {
  presets: HashMap<String, AgentPresetConfig>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct BuiltinAgentConfigFile {
  #[serde(flatten)]
  presets: HashMap<String, AgentPresetConfig>,
}

fn builtin_agent_config() -> Result<BuiltinAgentConfig> {
  let mut config = BuiltinAgentConfig::default();

  for (path, contents) in BUILTIN_AGENT_PRESET_CONFIGS {
    let file_config: BuiltinAgentConfigFile = serde_yaml::from_str(contents)
      .with_context(|| format!("failed to parse bundled {path}"))?;

    for (name, preset) in file_config.presets {
      if config.presets.insert(name.clone(), preset).is_some() {
        bail!("bundled agent preset '{name}' is defined more than once");
      }
    }
  }

  Ok(config)
}

fn builtin_agent_presets() -> Result<HashMap<String, AgentPresetConfig>> {
  Ok(builtin_agent_config()?.presets)
}

pub fn available_agent_presets(
  user_presets: &HashMap<String, AgentPresetConfig>,
) -> Result<Vec<String>> {
  let mut names: HashSet<String> = builtin_agent_presets()?
    .into_iter()
    .filter_map(|(name, preset)| preset.show_in_switcher.then_some(name))
    .collect();
  names.extend(
    user_presets
      .iter()
      .filter(|(_, preset)| preset.show_in_switcher)
      .map(|(name, _)| name.clone()),
  );
  let mut names: Vec<_> = names.into_iter().collect();
  names.sort();
  Ok(names)
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
      uses_mcp: config.uses_mcp.unwrap_or(false),
      uses_tool_cli: config.uses_tool_cli.unwrap_or(true),
      prompt_template: DEFAULT_PROMPT_TEMPLATE.to_string(),
    }
  };

  if let Some(command) = &config.command {
    resolved.command = command.clone();
  }
  if !config.args.is_empty() {
    resolved.args = config.args.clone();
  }
  resolved.args.extend(config.extra_args.clone());
  if let Some(uses_mcp) = config.uses_mcp {
    resolved.uses_mcp = uses_mcp;
  }
  if let Some(uses_tool_cli) = config.uses_tool_cli {
    resolved.uses_tool_cli = uses_tool_cli;
  }
  if let Some(prompt_template) = &config.prompt_template {
    resolved.prompt_template = prompt_template.clone();
  }

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
      uses_mcp: false,
      uses_tool_cli: true,
      prompt_template: DEFAULT_PROMPT_TEMPLATE.to_string(),
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
  if let Some(uses_mcp) = preset.uses_mcp {
    resolved.uses_mcp = uses_mcp;
  }
  if let Some(uses_tool_cli) = preset.uses_tool_cli {
    resolved.uses_tool_cli = uses_tool_cli;
  }
  if let Some(prompt_template) = preset.prompt_template {
    resolved.prompt_template = prompt_template;
  }

  if resolved.command.is_empty() {
    bail!("agent preset '{name}' does not define a command");
  }

  seen.remove(name);
  Ok(resolved)
}

#[derive(Debug, Serialize)]
struct AgentLaunchTemplateData<'a> {
  cwd: String,
  mcp_url: &'a str,
  tool_command: &'a str,
  mcp_command: &'a str,
  mcp_port: &'a str,
  context_prompt: &'a str,
  uses_mcp: bool,
  uses_tool_cli: bool,
}

impl<'a> AgentLaunchTemplateData<'a> {
  fn new(
    context: &'a AgentLaunchContext,
    context_prompt: &'a str,
    uses_mcp: bool,
    uses_tool_cli: bool,
  ) -> Self {
    Self {
      cwd: context.cwd.display().to_string(),
      mcp_url: &context.mcp_url,
      tool_command: &context.terminai_tool_command,
      mcp_command: &context.terminai_mcp_command,
      mcp_port: &context.terminai_mcp_port,
      context_prompt,
      uses_mcp: uses_mcp,
      uses_tool_cli: uses_tool_cli,
    }
  }
}

fn render_context_prompt(
  environment: &Environment<'_>,
  context: &AgentLaunchContext,
  template_name: &str,
  uses_mcp: bool,
  uses_tool_cli: bool,
) -> Result<String> {
  let data = AgentLaunchTemplateData::new(context, "", uses_mcp, uses_tool_cli);
  environment
    .get_template(template_name)
    .with_context(|| {
      format!("failed to load prompt template '{template_name}'")
    })?
    .render(&data)
    .with_context(|| {
      format!("failed to render prompt template '{template_name}'")
    })
}

fn expand_args(
  environment: &Environment<'_>,
  args: Vec<AgentArg>,
  context: &AgentLaunchContext,
  context_prompt: &str,
  uses_mcp: bool,
  uses_tool_cli: bool,
) -> Result<Vec<String>> {
  let data = AgentLaunchTemplateData::new(
    context,
    context_prompt,
    uses_mcp,
    uses_tool_cli,
  );
  let mut expanded = Vec::new();

  for arg in args {
    match arg {
      AgentArg::Template(template) => {
        let rendered = environment
          .render_named_str("<agent-arg>", &template, &data)
          .with_context(|| {
            format!("failed to render agent arg template: {template}")
          })?;
        expanded.push(rendered);
      }
      AgentArg::Expression { expr } => {
        let value = environment
          .compile_expression_owned(expr.clone())
          .with_context(|| {
            format!("failed to compile agent arg expression: {expr}")
          })?
          .eval(&data)
          .with_context(|| {
            format!("failed to evaluate agent arg expression: {expr}")
          })?;
        let values = Vec::<String>::deserialize(value).with_context(|| {
          format!(
            "agent arg expression must produce an array of strings: {expr}"
          )
        })?;
        expanded.extend(values);
      }
    }
  }

  Ok(expanded)
}

fn launch_template_environment(
  config_dir: Option<PathBuf>,
) -> Environment<'static> {
  let mut environment = Environment::new();
  environment.set_undefined_behavior(UndefinedBehavior::Strict);
  environment.set_auto_escape_callback(|_| AutoEscape::None);
  environment.set_keep_trailing_newline(true);
  environment.add_filter("toml", toml_string);
  environment.add_filter("json", json_string);
  environment
    .set_loader(move |name| load_template(config_dir.as_deref(), name));
  environment
}

fn load_template(
  config_dir: Option<&Path>,
  name: &str,
) -> std::result::Result<Option<String>, MinijinjaError> {
  if !is_safe_template_name(name) {
    return Err(MinijinjaError::new(
      MinijinjaErrorKind::InvalidOperation,
      format!(
        "template name must stay within the Terminai config directory: {name}"
      ),
    ));
  }

  if name == BUILTIN_DEFAULT_PROMPT_TEMPLATE {
    return Ok(Some(BUILTIN_DEFAULT_PROMPT.to_string()));
  }

  if let Some(config_dir) = config_dir
    && let Some(source) = load_user_template(config_dir, name)?
  {
    return Ok(Some(source));
  }

  if name == DEFAULT_PROMPT_TEMPLATE {
    Ok(Some(BUILTIN_DEFAULT_PROMPT.to_string()))
  } else {
    Ok(None)
  }
}

fn is_safe_template_name(name: &str) -> bool {
  !name.is_empty()
    && Path::new(name)
      .components()
      .all(|component| matches!(component, Component::Normal(_)))
}

fn load_user_template(
  config_dir: &Path,
  name: &str,
) -> std::result::Result<Option<String>, MinijinjaError> {
  let config_dir = match std::fs::canonicalize(config_dir) {
    Ok(path) => path,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
    Err(err) => {
      return Err(
        MinijinjaError::new(
          MinijinjaErrorKind::InvalidOperation,
          "failed to resolve Terminai config directory",
        )
        .with_source(err),
      );
    }
  };
  let candidate = match std::fs::canonicalize(config_dir.join(name)) {
    Ok(path) => path,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
    Err(err) => {
      return Err(
        MinijinjaError::new(
          MinijinjaErrorKind::InvalidOperation,
          format!("failed to resolve prompt template '{name}'"),
        )
        .with_source(err),
      );
    }
  };
  if !candidate.starts_with(&config_dir) {
    return Err(MinijinjaError::new(
      MinijinjaErrorKind::InvalidOperation,
      format!(
        "prompt template '{name}' resolves outside the Terminai config directory"
      ),
    ));
  }
  std::fs::read_to_string(candidate).map(Some).map_err(|err| {
    MinijinjaError::new(
      MinijinjaErrorKind::InvalidOperation,
      format!("failed to read prompt template '{name}'"),
    )
    .with_source(err)
  })
}

fn toml_string(value: &str) -> String {
  format!("{value:?}")
}

fn json_string(value: &str) -> String {
  serde_json::to_string(value).expect("string serialization cannot fail")
}

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::TempDir;

  fn context() -> AgentLaunchContext {
    let mut context = AgentLaunchContext::new(
      PathBuf::from("/tmp/project"),
      "http://127.0.0.1:3456/mcp".to_string(),
      "test-token".to_string(),
      "/usr/bin/terminai".to_string(),
      "3456".to_string(),
    );
    context.config_dir = None;
    context
  }

  fn context_in(config_dir: &Path) -> AgentLaunchContext {
    let mut context = context();
    context.config_dir = Some(config_dir.to_path_buf());
    context
  }

  fn custom_agent_with_prompt(template: Option<&str>) -> AgentConfig {
    AgentConfig {
      preset: None,
      kind: Some(AgentKind::Custom),
      command: Some("my-agent".to_string()),
      args: vec!["{{ context_prompt }}".into()],
      prompt_template: template.map(str::to_string),
      ..Default::default()
    }
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
    assert!(BUILTIN_DEFAULT_PROMPT.contains("check_for_updates"));
    assert!(BUILTIN_DEFAULT_PROMPT.contains("{% block introduction %}"));
  }

  #[test]
  fn switcher_lists_builtins_and_visible_user_presets() {
    let mut presets = HashMap::new();
    presets.insert(
      "visible".to_string(),
      AgentPresetConfig {
        command: Some("visible-agent".to_string()),
        ..Default::default()
      },
    );
    presets.insert(
      "hidden".to_string(),
      AgentPresetConfig {
        command: Some("hidden-agent".to_string()),
        show_in_switcher: false,
        ..Default::default()
      },
    );

    let names = available_agent_presets(&presets).unwrap();

    assert_eq!(names, ["claude", "codex", "opencode", "visible"]);
  }

  #[test]
  fn codex_plan_passes_extremely_clear_e2e_instructions() {
    let config = AgentConfig::codex();
    let plan = build_launch_plan(&config, &HashMap::new(), &context()).unwrap();

    assert_eq!(plan.command, "codex");
    assert_eq!(
      plan.env.get("TERMINAI_MCP_AUTH_TOKEN").map(String::as_str),
      Some("test-token")
    );
    assert_eq!(
      plan.env.get("TERMINAI_MCP_PORT").map(String::as_str),
      Some("3456")
    );
    assert!(!plan.env.contains_key("TERMINAI_MCP_URL"));
    assert!(!plan.env.contains_key("TERMINAI_MCP_COMMAND"));
    assert!(!plan.env.contains_key("TERMINAI_CONTEXT_PROMPT"));
    assert_eq!(plan.metadata.mcp_url, "http://127.0.0.1:3456/mcp");
    assert_eq!(plan.metadata.mcp_auth_token, "test-token");
    assert_eq!(plan.metadata.terminai_binary_path, "/usr/bin/terminai");
    assert_eq!(
      plan.metadata.terminai_tool_command,
      "/usr/bin/terminai tool"
    );
    assert_eq!(plan.metadata.terminai_mcp_command, "/usr/bin/terminai _mcp");
    assert_eq!(plan.metadata.terminai_mcp_port, "3456");
    assert!(plan.args.contains(&"--no-alt-screen".to_string()));
    assert!(plan.args.iter().any(|arg| arg.contains("mcp_servers")));
    assert!(plan.args.iter().any(
      |arg| arg == "mcp_servers.terminai.url=\"http://127.0.0.1:3456/mcp\""
    ));
    assert!(
      plan
        .args
        .iter()
        .any(|arg| arg
          == "mcp_servers.terminai.bearer_token_env_var=\"TERMINAI_MCP_AUTH_TOKEN\"")
    );
    assert!(!plan.args.iter().any(|arg| arg.contains("_mcp")));
    assert!(!plan.args.iter().any(|arg| arg.contains(".command=")));
    assert!(!plan.args.iter().any(|arg| arg.contains(".env.")));
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
    assert!(!developer_instructions.contains("{% block"));
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
    let mcp_config = plan
      .args
      .iter()
      .find(|arg| arg.contains("mcpServers"))
      .unwrap();
    assert!(mcp_config.contains("\"type\":\"http\""));
    assert!(mcp_config.contains("\"url\":\"http://127.0.0.1:3456/mcp\""));
    assert!(mcp_config.contains(
      "\"headers\":{\"Authorization\":\"Bearer ${TERMINAI_MCP_AUTH_TOKEN}\"}"
    ));
    assert!(!mcp_config.contains("\"type\":\"stdio\""));
    assert!(!mcp_config.contains("\"command\""));
    assert!(!mcp_config.contains("\"_mcp\""));
  }

  #[test]
  fn custom_plan_expands_templates() {
    let config = AgentConfig {
      preset: None,
      kind: Some(AgentKind::Custom),
      command: Some("my-agent".to_string()),
      args: vec![
        "--url={{ mcp_url }}".into(),
        "--cwd={{ cwd }}".into(),
        "--tool={{ tool_command }}".into(),
        "--mcp-command={{ mcp_command }}".into(),
        "--json-command={{ mcp_command|json }}".into(),
        "--toml-port={{ mcp_port|toml }}".into(),
        "--uses-mcp={{ uses_mcp }}".into(),
        "--uses-cli={{ uses_tool_cli }}".into(),
        "_mcp".into(),
      ],
      extra_args: Vec::new(),
      prompt_template: None,
      uses_mcp: None,
      uses_tool_cli: None,
    };
    let plan = build_launch_plan(&config, &HashMap::new(), &context()).unwrap();

    assert_eq!(plan.command, "my-agent");
    assert_eq!(plan.args[0], "--url=http://127.0.0.1:3456/mcp");
    assert_eq!(plan.args[1], "--cwd=/tmp/project");
    assert_eq!(plan.args[2], "--tool=/usr/bin/terminai tool");
    assert_eq!(plan.args[3], "--mcp-command=/usr/bin/terminai _mcp");
    assert_eq!(plan.args[4], "--json-command=\"/usr/bin/terminai _mcp\"");
    assert_eq!(plan.args[5], "--toml-port=\"3456\"");
    assert_eq!(plan.args[6], "--uses-mcp=false");
    assert_eq!(plan.args[7], "--uses-cli=true");
    assert_eq!(plan.args[8], "_mcp");
    assert_eq!(
      plan.env.get("TERMINAI_MCP_PORT").map(String::as_str),
      Some("3456")
    );
  }

  #[test]
  fn custom_agent_defaults_to_cli_without_mcp() {
    let config = AgentConfig {
      preset: None,
      kind: Some(AgentKind::Custom),
      command: Some("my-agent".to_string()),
      args: vec!["{{ context_prompt }}".into()],
      ..Default::default()
    };

    let plan = build_launch_plan(&config, &HashMap::new(), &context()).unwrap();

    assert_eq!(plan.command, "my-agent");
    assert!(!plan.args.iter().any(|arg| arg.contains("mcp_servers")));
    assert!(plan.args[0].contains("/usr/bin/terminai tool"));
    assert!(plan.args[0].contains("/usr/bin/terminai tool check_for_updates"));
    assert!(!plan.args[0].contains("Terminai MCP tool"));
  }

  #[test]
  fn agent_config_can_disable_bundled_preset_mcp_args() {
    let config = AgentConfig {
      preset: Some("codex".to_string()),
      uses_mcp: Some(false),
      uses_tool_cli: Some(true),
      ..Default::default()
    };

    let plan = build_launch_plan(&config, &HashMap::new(), &context()).unwrap();

    assert_eq!(plan.command, "codex");
    assert!(plan.args.contains(&"--no-alt-screen".to_string()));
    assert!(!plan.args.iter().any(|arg| arg.contains("mcp_servers")));
    assert!(
      plan
        .args
        .iter()
        .any(|arg| arg.contains("/usr/bin/terminai tool check_for_updates"))
    );
  }

  #[test]
  fn custom_plan_uses_jinja_expressions_without_html_escaping() {
    let mut context = context();
    context.mcp_url = "http://127.0.0.1:3456/<&>\"'".to_string();
    let config = AgentConfig {
      preset: None,
      kind: Some(AgentKind::Custom),
      command: Some("my-agent".to_string()),
      args: vec!["{% if mcp_url %}url={{ mcp_url }}{% endif %}".into()],
      ..Default::default()
    };

    let plan = build_launch_plan(&config, &HashMap::new(), &context).unwrap();

    assert_eq!(plan.args[0], "url=http://127.0.0.1:3456/<&>\"'");
  }

  #[test]
  fn custom_plan_flattens_expression_args() {
    let config = AgentConfig {
      preset: None,
      kind: Some(AgentKind::Custom),
      command: Some("my-agent".to_string()),
      args: vec![
        "before".into(),
        AgentArg::Expression {
          expr: r#"["--cwd=" ~ cwd, "port=" ~ mcp_port]"#.to_string(),
        },
        "after".into(),
      ],
      ..Default::default()
    };

    let plan = build_launch_plan(&config, &HashMap::new(), &context()).unwrap();

    assert_eq!(
      plan.args,
      vec![
        "before".to_string(),
        "--cwd=/tmp/project".to_string(),
        "port=3456".to_string(),
        "after".to_string(),
      ]
    );
  }

  #[test]
  fn custom_plan_expression_can_produce_zero_arguments() {
    let config = AgentConfig {
      preset: None,
      kind: Some(AgentKind::Custom),
      command: Some("my-agent".to_string()),
      args: vec![
        "before".into(),
        AgentArg::Expression {
          expr: "[]".to_string(),
        },
        "after".into(),
      ],
      ..Default::default()
    };

    let plan = build_launch_plan(&config, &HashMap::new(), &context()).unwrap();

    assert_eq!(plan.args, vec!["before".to_string(), "after".to_string()]);
  }

  #[test]
  fn custom_plan_rejects_non_array_expression_results() {
    let config = AgentConfig {
      preset: None,
      kind: Some(AgentKind::Custom),
      command: Some("my-agent".to_string()),
      args: vec![AgentArg::Expression {
        expr: r#""one""#.to_string(),
      }],
      ..Default::default()
    };

    let err = build_launch_plan(&config, &HashMap::new(), &context())
      .expect_err("string expression result should fail");

    assert!(err.to_string().contains("must produce an array of strings"));
  }

  #[test]
  fn custom_plan_rejects_non_string_expression_items() {
    let config = AgentConfig {
      preset: None,
      kind: Some(AgentKind::Custom),
      command: Some("my-agent".to_string()),
      args: vec![AgentArg::Expression {
        expr: r#"["one", 2]"#.to_string(),
      }],
      ..Default::default()
    };

    let err = build_launch_plan(&config, &HashMap::new(), &context())
      .expect_err("non-string expression item should fail");

    assert!(err.to_string().contains("must produce an array of strings"));
  }

  #[test]
  fn custom_plan_preserves_empty_template_arguments() {
    let config = AgentConfig {
      preset: None,
      kind: Some(AgentKind::Custom),
      command: Some("my-agent".to_string()),
      args: vec!["".into()],
      ..Default::default()
    };

    let plan = build_launch_plan(&config, &HashMap::new(), &context()).unwrap();

    assert_eq!(plan.args, vec![String::new()]);
  }

  #[test]
  fn custom_plan_rejects_unknown_template_variables() {
    let config = AgentConfig {
      preset: None,
      kind: Some(AgentKind::Custom),
      command: Some("my-agent".to_string()),
      args: vec!["--bad={{mc_url}}".into()],
      ..Default::default()
    };

    let err = build_launch_plan(&config, &HashMap::new(), &context())
      .expect_err("unknown template variable should fail");

    assert!(
      err
        .to_string()
        .contains("failed to render agent arg template")
    );
  }

  #[test]
  fn custom_plan_rejects_unknown_template_filters() {
    let config = AgentConfig {
      preset: None,
      kind: Some(AgentKind::Custom),
      command: Some("my-agent".to_string()),
      args: vec!["--bad={{ mcp_url|missing_filter }}".into()],
      ..Default::default()
    };

    let err = build_launch_plan(&config, &HashMap::new(), &context())
      .expect_err("unknown template filter should fail");

    assert!(
      err
        .to_string()
        .contains("failed to render agent arg template")
    );
  }

  #[test]
  fn user_default_prompt_shadows_builtin_default() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
      dir.path().join(DEFAULT_PROMPT_TEMPLATE),
      "User default for {{ tool_command }}",
    )
    .unwrap();

    let plan = build_launch_plan(
      &custom_agent_with_prompt(None),
      &HashMap::new(),
      &context_in(dir.path()),
    )
    .unwrap();

    assert_eq!(plan.args, ["User default for /usr/bin/terminai tool"]);
  }

  #[test]
  fn user_default_prompt_can_extend_builtin_default() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
      dir.path().join(DEFAULT_PROMPT_TEMPLATE),
      r#"{% extends "builtin/default.jinja" %}{% block introduction %}User introduction.
{% endblock %}"#,
    )
    .unwrap();

    let plan = build_launch_plan(
      &custom_agent_with_prompt(None),
      &HashMap::new(),
      &context_in(dir.path()),
    )
    .unwrap();

    assert!(plan.args[0].contains("User introduction."));
    assert!(plan.args[0].contains("Important Terminai rules:"));
  }

  #[test]
  fn selected_prompt_extends_user_shadowed_default() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
      dir.path().join(DEFAULT_PROMPT_TEMPLATE),
      r#"{% extends "builtin/default.jinja" %}{% block introduction %}User introduction.
{% endblock %}"#,
    )
    .unwrap();
    std::fs::write(
      dir.path().join("custom.jinja"),
      r#"{% extends "default.jinja" %}{% block general_rules %}Custom rules.
{% endblock %}"#,
    )
    .unwrap();

    let plan = build_launch_plan(
      &custom_agent_with_prompt(Some("custom.jinja")),
      &HashMap::new(),
      &context_in(dir.path()),
    )
    .unwrap();

    assert!(plan.args[0].contains("User introduction."));
    assert!(plan.args[0].contains("Custom rules."));
    assert!(!plan.args[0].contains("base assumption"));
  }

  #[test]
  fn prompt_template_is_inherited_from_preset_and_overridden_by_agent() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("preset.jinja"), "Preset prompt").unwrap();
    std::fs::write(dir.path().join("agent.jinja"), "Agent prompt").unwrap();
    let mut presets = HashMap::new();
    presets.insert(
      "custom".to_string(),
      AgentPresetConfig {
        command: Some("my-agent".to_string()),
        args: vec!["{{ context_prompt }}".into()],
        prompt_template: Some("preset.jinja".to_string()),
        ..Default::default()
      },
    );
    let mut config = AgentConfig {
      preset: Some("custom".to_string()),
      ..Default::default()
    };

    let preset_plan =
      build_launch_plan(&config, &presets, &context_in(dir.path())).unwrap();
    assert_eq!(preset_plan.args, ["Preset prompt"]);

    config.prompt_template = Some("agent.jinja".to_string());
    let agent_plan =
      build_launch_plan(&config, &presets, &context_in(dir.path())).unwrap();

    assert_eq!(agent_plan.args, ["Agent prompt"]);
  }

  #[test]
  fn prompt_template_cannot_escape_config_directory() {
    let parent = TempDir::new().unwrap();
    let config_dir = parent.path().join("config");
    std::fs::create_dir(&config_dir).unwrap();
    std::fs::write(parent.path().join("outside.jinja"), "Outside").unwrap();

    let err = build_launch_plan(
      &custom_agent_with_prompt(Some("../outside.jinja")),
      &HashMap::new(),
      &context_in(&config_dir),
    )
    .expect_err("template traversal should fail");

    assert!(err.to_string().contains("failed to load prompt template"));
    assert!(format!("{err:#}").contains("must stay within"));
  }

  #[test]
  fn user_preset_can_extend_builtin_and_append_flags() {
    let mut presets = HashMap::new();
    presets.insert(
      "codex-fast".to_string(),
      AgentPresetConfig {
        extends: Some("codex".to_string()),
        extra_args: vec!["--model".into(), "gpt-5.5".into()],
        ..Default::default()
      },
    );
    let config = AgentConfig {
      preset: Some("codex-fast".to_string()),
      extra_args: vec!["--search".into()],
      ..Default::default()
    };

    let plan = build_launch_plan(&config, &presets, &context()).unwrap();

    assert_eq!(plan.command, "codex");
    assert!(plan.args.contains(&"--no-alt-screen".to_string()));
    assert!(plan.args.iter().any(|arg| arg.contains("mcp_servers")));
    assert_eq!(
      &plan.args[plan.args.len() - 3..],
      ["--model", "gpt-5.5", "--search"]
    );
  }
}
