use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{Context as AnyhowContext, Result, bail};
use handlebars::{
  Context as HandlebarsContext, Handlebars, Helper, HelperDef, HelperResult,
  JsonRender, Output, RenderContext, RenderError, RenderErrorReason,
  Renderable, StringOutput, no_escape,
};
use serde::{Deserialize, Serialize};

use crate::terminai_config::{AgentConfig, AgentKind, AgentPresetConfig};

const ARGS_SENTINEL_VALUE: &str = "__TERMINAI_AGENT_ARGS_SENTINEL_5D1A2E49__";

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
  pub context_prompt: String,
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
      context_prompt: builtin_context_prompt(),
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
  let rendered_context_prompt =
    render_context_prompt(context, resolved.uses_mcp, resolved.uses_tool_cli)?;
  let args = expand_args(
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
  args: Vec<String>,
  env: HashMap<String, String>,
  uses_mcp: bool,
  uses_tool_cli: bool,
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
      uses_mcp: config.uses_mcp.unwrap_or(false),
      uses_tool_cli: config.uses_tool_cli.unwrap_or(true),
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
  context: &AgentLaunchContext,
  uses_mcp: bool,
  uses_tool_cli: bool,
) -> Result<String> {
  let handlebars = launch_arg_handlebars();
  let data = AgentLaunchTemplateData::new(
    context,
    &context.context_prompt,
    uses_mcp,
    uses_tool_cli,
  );
  handlebars
    .render_template(&context.context_prompt, &data)
    .context("failed to render agent context prompt template")
}

fn expand_args(
  args: Vec<String>,
  context: &AgentLaunchContext,
  context_prompt: &str,
  uses_mcp: bool,
  uses_tool_cli: bool,
) -> Result<Vec<String>> {
  let handlebars = launch_arg_handlebars();
  let data = AgentLaunchTemplateData::new(
    context,
    context_prompt,
    uses_mcp,
    uses_tool_cli,
  );
  let mut expanded = Vec::new();

  for arg in args {
    let rendered = handlebars
      .render_template(&arg, &data)
      .with_context(|| format!("failed to render agent arg template: {arg}"))?;
    expanded.extend(expand_rendered_arg(rendered, &arg)?);
  }

  Ok(expanded)
}

fn expand_rendered_arg(
  rendered: String,
  template: &str,
) -> Result<Vec<String>> {
  let trimmed_start = rendered.trim_start();
  if let Some(remainder) = trimmed_start.strip_prefix(ARGS_SENTINEL_VALUE) {
    if remainder.contains(ARGS_SENTINEL_VALUE) {
      bail!(
        "ARGS sentinel may only appear once at the beginning of an agent arg template"
      );
    }
    return serde_json::from_str::<Vec<String>>(remainder).with_context(|| {
      format!("failed to parse rendered agent arg array template: {template}")
    });
  }

  if rendered.contains(ARGS_SENTINEL_VALUE) {
    bail!(
      "ARGS sentinel may only appear at the beginning of an agent arg template"
    );
  }

  Ok(vec![rendered])
}

fn render_helper_template<'reg: 'rc, 'rc>(
  h: &Helper<'rc>,
  r: &'reg Handlebars<'reg>,
  ctx: &'rc HandlebarsContext,
  rc: &mut RenderContext<'reg, 'rc>,
) -> std::result::Result<String, RenderError> {
  let Some(template) = h.template() else {
    return Ok(String::new());
  };
  let mut output = StringOutput::new();
  template.render(r, ctx, rc, &mut output)?;
  Ok(output.into_string().expect("string output is always utf-8"))
}

fn strip_single_trailing_comma(value: &str) -> &str {
  let trimmed = value.trim_end();
  trimmed.strip_suffix(',').unwrap_or(trimmed)
}

struct ArgsHelper;

impl HelperDef for ArgsHelper {
  fn call<'reg: 'rc, 'rc>(
    &self,
    h: &Helper<'rc>,
    r: &'reg Handlebars<'reg>,
    ctx: &'rc HandlebarsContext,
    rc: &mut RenderContext<'reg, 'rc>,
    out: &mut dyn Output,
  ) -> HelperResult {
    let body = render_helper_template(h, r, ctx, rc)?;
    out.write(ARGS_SENTINEL_VALUE)?;
    out.write("[")?;
    out.write(strip_single_trailing_comma(&body))?;
    out.write("]")?;
    Ok(())
  }
}

struct ArgHelper;

impl HelperDef for ArgHelper {
  fn call<'reg: 'rc, 'rc>(
    &self,
    h: &Helper<'rc>,
    r: &'reg Handlebars<'reg>,
    ctx: &'rc HandlebarsContext,
    rc: &mut RenderContext<'reg, 'rc>,
    out: &mut dyn Output,
  ) -> HelperResult {
    let body = render_helper_template(h, r, ctx, rc)?;
    out.write(&json_string(&body))?;
    out.write(",")?;
    Ok(())
  }
}

fn omit_helper(
  _: &Helper<'_>,
  _: &Handlebars<'_>,
  _: &HandlebarsContext,
  _: &mut RenderContext<'_, '_>,
  out: &mut dyn Output,
) -> HelperResult {
  out.write(ARGS_SENTINEL_VALUE)?;
  out.write("[]")?;
  Ok(())
}

fn launch_arg_handlebars() -> Handlebars<'static> {
  let mut handlebars = Handlebars::new();
  handlebars.set_strict_mode(true);
  handlebars.register_escape_fn(no_escape);
  handlebars.register_helper("toml", Box::new(toml_helper));
  handlebars.register_helper("json", Box::new(json_helper));
  handlebars.register_helper("args", Box::new(ArgsHelper));
  handlebars.register_helper("arg", Box::new(ArgHelper));
  handlebars.register_helper("OMIT", Box::new(omit_helper));
  handlebars
}

fn toml_helper(
  h: &Helper<'_>,
  r: &Handlebars<'_>,
  _: &HandlebarsContext,
  _: &mut RenderContext<'_, '_>,
  out: &mut dyn Output,
) -> HelperResult {
  let param = h
    .param(0)
    .filter(|param| !r.strict_mode() || !param.is_value_missing())
    .ok_or(RenderErrorReason::ParamNotFoundForIndex("toml", 0))?;
  out.write(&toml_string(param.value().render().as_ref()))?;
  Ok(())
}

fn json_helper(
  h: &Helper<'_>,
  r: &Handlebars<'_>,
  _: &HandlebarsContext,
  _: &mut RenderContext<'_, '_>,
  out: &mut dyn Output,
) -> HelperResult {
  let param = h
    .param(0)
    .filter(|param| !r.strict_mode() || !param.is_value_missing())
    .ok_or(RenderErrorReason::ParamNotFoundForIndex("json", 0))?;
  out.write(&json_string(param.value().render().as_ref()))?;
  Ok(())
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

  fn context() -> AgentLaunchContext {
    AgentLaunchContext::new(
      PathBuf::from("/tmp/project"),
      "http://127.0.0.1:3456/mcp".to_string(),
      "test-token".to_string(),
      "/usr/bin/terminai".to_string(),
      "3456".to_string(),
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
        "--url={{mcp_url}}".to_string(),
        "--cwd={{cwd}}".to_string(),
        "--tool={{tool_command}}".to_string(),
        "--mcp-command={{mcp_command}}".to_string(),
        "--json-command={{json mcp_command}}".to_string(),
        "--toml-port={{toml mcp_port}}".to_string(),
        "--uses-mcp={{uses_mcp}}".to_string(),
        "--uses-cli={{uses_tool_cli}}".to_string(),
        "_mcp".to_string(),
      ],
      extra_args: Vec::new(),
      initial_prompt: None,
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
      args: vec!["{{context_prompt}}".to_string()],
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
  fn custom_plan_uses_handlebars_expressions_without_html_escaping() {
    let mut context = context();
    context.mcp_url = "http://127.0.0.1:3456/<&>\"'".to_string();
    let config = AgentConfig {
      preset: None,
      kind: Some(AgentKind::Custom),
      command: Some("my-agent".to_string()),
      args: vec!["{{#if mcp_url}}url={{mcp_url}}{{/if}}".to_string()],
      ..Default::default()
    };

    let plan = build_launch_plan(&config, &HashMap::new(), &context).unwrap();

    assert_eq!(plan.args[0], "url=http://127.0.0.1:3456/<&>\"'");
  }

  #[test]
  fn custom_plan_flattens_args_block_templates() {
    let config = AgentConfig {
      preset: None,
      kind: Some(AgentKind::Custom),
      command: Some("my-agent".to_string()),
      args: vec![
        "before".to_string(),
        "{{#args}}{{#arg}}--cwd={{cwd}}{{/arg}}{{#arg}}port={{mcp_port}}{{/arg}}{{/args}}".to_string(),
        "after".to_string(),
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
  fn custom_plan_strips_one_trailing_comma_from_args_block() {
    let config = AgentConfig {
      preset: None,
      kind: Some(AgentKind::Custom),
      command: Some("my-agent".to_string()),
      args: vec![
        "{{#args}}{{#arg}}one{{/arg}}{{#arg}}two{{/arg}}{{/args}}".to_string(),
      ],
      ..Default::default()
    };

    let plan = build_launch_plan(&config, &HashMap::new(), &context()).unwrap();

    assert_eq!(plan.args, vec!["one".to_string(), "two".to_string()]);
  }

  #[test]
  fn custom_plan_omit_helper_produces_zero_arguments() {
    let config = AgentConfig {
      preset: None,
      kind: Some(AgentKind::Custom),
      command: Some("my-agent".to_string()),
      args: vec![
        "before".to_string(),
        "{{OMIT}}".to_string(),
        "after".to_string(),
      ],
      ..Default::default()
    };

    let plan = build_launch_plan(&config, &HashMap::new(), &context()).unwrap();

    assert_eq!(plan.args, vec!["before".to_string(), "after".to_string()]);
  }

  #[test]
  fn custom_plan_rejects_misplaced_args_sentinel() {
    let config = AgentConfig {
      preset: None,
      kind: Some(AgentKind::Custom),
      command: Some("my-agent".to_string()),
      args: vec!["prefix {{#args}}{{/args}}".to_string()],
      ..Default::default()
    };

    let err = build_launch_plan(&config, &HashMap::new(), &context())
      .expect_err("misplaced args sentinel should fail");

    assert!(err.to_string().contains("ARGS sentinel"));
  }

  #[test]
  fn custom_plan_allows_leading_whitespace_before_args_sentinel() {
    let config = AgentConfig {
      preset: None,
      kind: Some(AgentKind::Custom),
      command: Some("my-agent".to_string()),
      args: vec![" \n\t{{#args}}{{#arg}}one{{/arg}}{{/args}}".to_string()],
      ..Default::default()
    };

    let plan = build_launch_plan(&config, &HashMap::new(), &context()).unwrap();

    assert_eq!(plan.args, vec!["one".to_string()]);
  }

  #[test]
  fn custom_plan_rejects_repeated_args_sentinel() {
    let config = AgentConfig {
      preset: None,
      kind: Some(AgentKind::Custom),
      command: Some("my-agent".to_string()),
      args: vec![format!("{{{{OMIT}}}} {ARGS_SENTINEL_VALUE}")],
      ..Default::default()
    };

    let err = build_launch_plan(&config, &HashMap::new(), &context())
      .expect_err("repeated args sentinel should fail");

    assert!(err.to_string().contains("ARGS sentinel"));
  }

  #[test]
  fn custom_plan_rejects_unknown_template_variables() {
    let config = AgentConfig {
      preset: None,
      kind: Some(AgentKind::Custom),
      command: Some("my-agent".to_string()),
      args: vec!["--bad={{mc_url}}".to_string()],
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
  fn custom_plan_rejects_unknown_template_helper_arguments() {
    let config = AgentConfig {
      preset: None,
      kind: Some(AgentKind::Custom),
      command: Some("my-agent".to_string()),
      args: vec!["--bad={{toml mc_url}}".to_string()],
      ..Default::default()
    };

    let err = build_launch_plan(&config, &HashMap::new(), &context())
      .expect_err("unknown template helper argument should fail");

    assert!(
      err
        .to_string()
        .contains("failed to render agent arg template")
    );
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
    assert!(plan.args.iter().any(|arg| arg.contains("mcp_servers")));
    assert_eq!(
      &plan.args[plan.args.len() - 3..],
      ["--model", "gpt-5.5", "--search"]
    );
  }
}
