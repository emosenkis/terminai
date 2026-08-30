use anyhow::{Context, Result, bail};

use crate::agent_launcher::AgentLaunchPlan;

pub fn is_prompt_start_escape(escape: &str) -> bool {
  ["\x1b]133;A", "\x1b]633;A"].iter().any(|prefix| {
    escape.strip_prefix(prefix).is_some_and(|rest| {
      rest.starts_with([';', '\x07']) || rest.starts_with("\x1b\\")
    })
  })
}

pub fn command_completion_prompt(terminal: &str) -> String {
  format!(
    "Suggest the next command for this terminal. Return only the exact shell input, with no Markdown or explanation. Do not include Enter, a newline, or any control character.\n\nTerminal:\n{terminal}"
  )
}

pub async fn run_completion(plan: AgentLaunchPlan) -> Result<String> {
  let mut command = tokio::process::Command::new(&plan.command);
  command
    .args(&plan.args)
    .envs(&plan.env)
    .current_dir(&plan.cwd)
    .kill_on_drop(true);
  let output =
    tokio::time::timeout(std::time::Duration::from_secs(30), command.output())
      .await
      .context("single-prompt agent invocation timed out")??;
  if !output.status.success() {
    bail!(
      "single-prompt agent invocation failed: {}",
      String::from_utf8_lossy(&output.stderr).trim()
    );
  }
  completion_text(&String::from_utf8(output.stdout)?)
    .context("single-prompt agent returned no safe completion")
}

pub fn completion_text(output: &str) -> Option<String> {
  let text = output.trim();
  (!text.is_empty() && !text.chars().any(char::is_control))
    .then(|| text.to_string())
}

pub fn current_completion(
  current_generation: u64,
  result_generation: u64,
  enabled: bool,
  result: std::result::Result<String, String>,
) -> Option<String> {
  (enabled && current_generation == result_generation)
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
    assert!(is_prompt_start_escape("\x1b]133;A\x07"));
    assert!(is_prompt_start_escape("\x1b]133;A\x1b\\"));
    assert!(is_prompt_start_escape("\x1b]633;A\x07"));
    assert!(is_prompt_start_escape("\x1b]633;A\x1b\\"));
    assert!(is_prompt_start_escape("\x1b]133;A;aid=42\x1b\\"));
    assert!(!is_prompt_start_escape("\x1b]133;B\x07"));
    assert!(!is_prompt_start_escape("\x1b]133;AB\x07"));
    assert!(!is_prompt_start_escape("\x1b]633;C\x07"));
  }

  #[test]
  fn accepts_only_non_executing_single_line_completions() {
    assert_eq!(
      completion_text(" git status \n").as_deref(),
      Some("git status")
    );
    assert_eq!(completion_text("```sh\ngit status\n```"), None);
    assert_eq!(completion_text("git status\rwhoami"), None);
    assert_eq!(completion_text("\x1b[31mrm -rf /"), None);
    assert_eq!(completion_text("   \n"), None);
  }

  #[test]
  fn prompt_requests_only_exact_non_executing_shell_input() {
    let prompt = command_completion_prompt("$ cargo test\nerror: failed");
    assert!(prompt.contains("$ cargo test\nerror: failed"));
    assert!(prompt.contains("exact shell input"));
    assert!(prompt.contains("Do not include Enter"));
  }

  #[tokio::test]
  async fn runs_the_configured_single_prompt_process() {
    #[cfg(windows)]
    let (command, args) =
      ("cmd.exe", vec!["/C".into(), "echo git status".into()]);
    #[cfg(not(windows))]
    let (command, args) =
      ("sh", vec!["-c".into(), "printf 'git status\\n'".into()]);
    let plan = AgentLaunchPlan {
      command: command.into(),
      args,
      env: HashMap::new(),
      cwd: std::env::temp_dir(),
      metadata: AgentLaunchMetadata::default(),
    };

    assert_eq!(run_completion(plan).await.unwrap(), "git status");
  }

  #[test]
  fn ignores_stale_or_disabled_results() {
    assert_eq!(
      current_completion(2, 2, true, Ok("git status".into())).as_deref(),
      Some("git status")
    );
    assert_eq!(current_completion(3, 2, true, Ok("stale".into())), None);
    assert_eq!(current_completion(2, 2, false, Ok("disabled".into())), None);
    assert_eq!(current_completion(2, 2, true, Err("failed".into())), None);
  }
}
