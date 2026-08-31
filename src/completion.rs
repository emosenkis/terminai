use anyhow::{Context, Result, bail};

use crate::agent_launcher::AgentLaunchPlan;

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

pub fn command_completion_prompt(terminal: &str) -> String {
  format!(
    "Complete the editable input at the final shell prompt. Return a JSON array containing up to three likely full command lines, best first. Return only the JSON array with no Markdown or explanation. Do not include Enter, a newline, or any control character inside a command. Every result must begin exactly with the current input.\n\nTerminal:\n{terminal}"
  )
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
    let value = value.trim();
    if !value.is_empty()
      && !value.chars().any(char::is_control)
      && !safe.iter().any(|existing| existing == value)
    {
      safe.push(value.to_string());
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
    assert_eq!(completion_texts("```sh\ngit status\n```"), None);
    assert_eq!(completion_texts("git status\rwhoami"), None);
    assert_eq!(completion_texts("\x1b[31mrm -rf /"), None);
    assert_eq!(completion_texts("   \n"), None);
  }

  #[test]
  fn prompt_requests_only_exact_non_executing_shell_input() {
    let prompt =
      command_completion_prompt("$ cargo test\nerror: failed\n$ git s");
    assert!(prompt.contains("$ cargo test\nerror: failed"));
    assert!(prompt.contains("$ git s"));
    assert!(prompt.contains("Do not include Enter"));
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
