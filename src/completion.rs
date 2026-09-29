use anyhow::{Context, Result, bail};
use minijinja::{Environment, UndefinedBehavior, context};

use crate::agent_launcher::AgentLaunchPlan;

const COMPLETION_PROMPT: &str = include_str!("../config/completion.jinja");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticPromptMarker {
  PromptStart,
  CommandStart,
  CommandExecuted,
  CommandFinished,
}

pub fn semantic_prompt_marker(escape: &str) -> Option<SemanticPromptMarker> {
  let rest = ["\x1b]133;", "\x1b]633;"]
    .iter()
    .find_map(|prefix| escape.strip_prefix(prefix))?;
  let marker = rest.chars().next()?;
  let rest = &rest[marker.len_utf8()..];
  if !(rest.starts_with([';', '\x07']) || rest.starts_with("\x1b\\")) {
    return None;
  }
  match marker {
    'A' => Some(SemanticPromptMarker::PromptStart),
    'B' => Some(SemanticPromptMarker::CommandStart),
    'C' => Some(SemanticPromptMarker::CommandExecuted),
    'D' => Some(SemanticPromptMarker::CommandFinished),
    _ => None,
  }
}

pub fn command_completion_prompt(terminal: &str, input: &str) -> String {
  let input_json = serde_json::to_string(input)
    .expect("serializing a string as JSON cannot fail");
  render_completion_prompt(
    context! { mode => "tracked-input", terminal, input_json },
  )
}

pub fn command_completion_suffix_prompt(terminal: &str) -> String {
  render_completion_prompt(context! { mode => "terminal-snapshot", terminal })
}

pub fn completion_with_prefix(input: &str, suggestion: &str) -> Option<String> {
  let suffix = suggestion
    .strip_prefix(input)
    .or_else(|| suggestion.trim_start().strip_prefix(input.trim_start()));
  let suffix = suffix.unwrap_or(suggestion);
  (!suffix.is_empty()).then(|| format!("{input}{suffix}"))
}

fn render_completion_prompt(context: minijinja::Value) -> String {
  let mut environment = Environment::new();
  environment.set_undefined_behavior(UndefinedBehavior::Strict);
  environment
    .render_str(COMPLETION_PROMPT, context)
    .expect("bundled completion prompt template must render")
}

pub async fn run_completion(plan: AgentLaunchPlan) -> Result<Vec<String>> {
  let mut command = tokio::process::Command::new(&plan.command);
  command
    .args(&plan.args)
    .envs(&plan.env)
    .current_dir(&plan.cwd)
    .kill_on_drop(true);
  let output =
    tokio::time::timeout(std::time::Duration::from_secs(30), command.output())
      .await
      .context("auto-completer invocation timed out")??;
  if !output.status.success() {
    bail!(
      "auto-completer invocation failed: {}",
      String::from_utf8_lossy(&output.stderr).trim()
    );
  }
  completion_texts(&String::from_utf8(output.stdout)?)
    .context("auto-completer returned no safe completion")
}

pub fn completion_texts(output: &str) -> Option<Vec<String>> {
  let text = output.trim();
  let values = serde_json::from_str::<Vec<String>>(text)
    .unwrap_or_else(|_| vec![text.to_string()]);
  let mut safe = Vec::new();
  for value in values {
    if !value.trim().is_empty()
      && !value.chars().any(char::is_control)
      && !safe.contains(&value)
    {
      safe.push(value);
    }
    if safe.len() == 3 {
      break;
    }
  }
  (!safe.is_empty()).then_some(safe)
}

pub fn current_completion(
  current_generation: u64,
  result_generation: u64,
  result: std::result::Result<Vec<String>, String>,
) -> Option<Vec<String>> {
  (current_generation == result_generation)
    .then(|| result.ok())
    .flatten()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::agent_launcher::AgentLaunchMetadata;
  use std::collections::HashMap;

  #[test]
  fn detects_common_semantic_prompt_markers() {
    use SemanticPromptMarker::*;
    assert_eq!(semantic_prompt_marker("\x1b]133;A\x07"), Some(PromptStart));
    assert_eq!(
      semantic_prompt_marker("\x1b]133;B\x1b\\"),
      Some(CommandStart)
    );
    assert_eq!(
      semantic_prompt_marker("\x1b]633;C\x07"),
      Some(CommandExecuted)
    );
    assert_eq!(
      semantic_prompt_marker("\x1b]633;D;0\x1b\\"),
      Some(CommandFinished)
    );
    assert_eq!(
      semantic_prompt_marker("\x1b]133;A;aid=42\x1b\\"),
      Some(PromptStart)
    );
    assert_eq!(semantic_prompt_marker("\x1b]133;AB\x07"), None);
    assert_eq!(semantic_prompt_marker("\x1b]133;"), None);
  }

  #[test]
  fn accepts_only_unique_non_executing_single_line_completions() {
    assert_eq!(
      completion_texts("[\"git status\",\"git diff\",\"git status\"]"),
      Some(vec!["git status".into(), "git diff".into()])
    );
    assert_eq!(
      completion_texts(" git status \n"),
      Some(vec!["git status".into()])
    );
    assert_eq!(
      completion_texts("[\" status\"]"),
      Some(vec![" status".into()])
    );
    assert_eq!(completion_texts("```sh\ngit status\n```"), None);
    assert_eq!(completion_texts("git status\rwhoami"), None);
    assert_eq!(completion_texts("\x1b[31mrm -rf /"), None);
    assert_eq!(completion_texts("   \n"), None);
  }

  #[test]
  fn prompt_requests_append_only_non_executing_shell_input() {
    let prompt = command_completion_prompt(
      "$ cargo test\nerror: failed\n$ git s",
      "git s",
    );
    assert!(prompt.contains("$ cargo test\nerror: failed"));
    assert!(prompt.contains("$ git s"));
    assert!(prompt.contains("<mode>tracked-input</mode>"));
    assert!(prompt.contains("<editable-input-json>\"git s\""));
    assert!(prompt.contains("exactly the characters to append"));
    assert!(prompt.contains("<example>"));
    assert!(!prompt.contains("<example "));

    let prompt = command_completion_suffix_prompt("$ git");
    assert!(prompt.contains("<mode>terminal-snapshot</mode>"));
    assert!(prompt.contains("<good>[\"tatus\",\"witch \"]</good>"));
  }

  #[test]
  fn suffix_results_tolerate_repeated_prefix_and_leading_whitespace() {
    assert_eq!(
      completion_with_prefix("git s", "tatus"),
      Some("git status".into())
    );
    assert_eq!(
      completion_with_prefix("git s", "git status"),
      Some("git status".into())
    );
    assert_eq!(
      completion_with_prefix("  git s", "git status"),
      Some("  git status".into())
    );
    assert_eq!(
      completion_with_prefix("git s", "   git status"),
      Some("git status".into())
    );
    assert_eq!(completion_with_prefix("git s", "git s"), None);
    assert_eq!(
      completion_with_prefix("", "git status"),
      Some("git status".into())
    );
  }

  #[tokio::test]
  async fn runs_the_configured_auto_completer() {
    #[cfg(windows)]
    let (command, args) =
      ("cmd.exe", vec!["/C".into(), "echo git status".into()]);
    #[cfg(not(windows))]
    let (command, args) = (
      "sh",
      vec!["-c".into(), "printf '[\"git status\"]\\n'".into()],
    );
    let plan = AgentLaunchPlan {
      command: command.into(),
      args,
      env: HashMap::new(),
      cwd: std::env::temp_dir(),
      metadata: AgentLaunchMetadata::default(),
    };

    assert_eq!(run_completion(plan).await.unwrap(), vec!["git status"]);
  }

  #[test]
  fn ignores_stale_or_disabled_results() {
    assert_eq!(
      current_completion(2, 2, Ok(vec!["git status".into()])),
      Some(vec!["git status".into()])
    );
    assert_eq!(current_completion(3, 2, Ok(vec!["stale".into()])), None);
    assert_eq!(current_completion(2, 2, Err("failed".into())), None);
  }
}
