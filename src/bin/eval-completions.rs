use anyhow::{Context, Result, bail};
use serde::Deserialize;
use termin::agent_launcher::{
  AgentLaunchContext, build_auto_completer_plan_for_request,
};
use termin::completion::{completion_with_prefix, run_completion};
use termin::terminai_config::TerminaiConfig;

const CASES: &str = include_str!("../../config/completion-eval.json");

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Case {
  name: String,
  trigger: String,
  terminal: String,
  input: Option<String>,
  accepted_completions: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
  let repeats = std::env::args()
    .nth(1)
    .unwrap_or_else(|| "3".into())
    .parse::<usize>()
    .context("repeat count must be a positive integer")?;
  if repeats == 0 {
    bail!("repeat count must be a positive integer");
  }

  let cases: Vec<Case> = serde_json::from_str(CASES)?;
  let config = TerminaiConfig::load()?;
  let cwd = std::env::current_dir()?;
  let context = AgentLaunchContext::new(
    cwd,
    String::new(),
    String::new(),
    std::env::current_exe()?.display().to_string(),
    String::new(),
  );
  let mut failures = 0;

  for case in &cases {
    for attempt in 1..=repeats {
      let plan = build_auto_completer_plan_for_request(
        config.completion_agent(),
        config.completion_agents(),
        &context,
        &case.terminal,
        case.input.as_deref(),
      )?;
      let suggestions = run_completion(plan).await?;
      let completions: Vec<String> = suggestions
        .iter()
        .filter_map(|suggestion| match &case.input {
          Some(input) => completion_with_prefix(input, suggestion),
          None => Some(suggestion.clone()),
        })
        .collect();
      let passed = completions
        .iter()
        .any(|completion| case.accepted_completions.contains(completion));
      println!(
        "{} {}/{} {} ({}): {:?}",
        if passed { "PASS" } else { "FAIL" },
        attempt,
        repeats,
        case.name,
        case.trigger,
        suggestions
      );
      failures += usize::from(!passed);
    }
  }

  if failures > 0 {
    bail!("{failures} completion evaluation(s) failed");
  }
  Ok(())
}
