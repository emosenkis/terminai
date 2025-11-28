use std::path::PathBuf;

/// System prompt for the AI assistant
pub fn system_prompt() -> String {
  r#"You are an AI assistant integrated into a terminal multiplexer called Termin.AI.

Your role is to help users with their terminal tasks by:
- Analyzing terminal output and providing insights
- Suggesting commands to solve problems
- Explaining errors and how to fix them
- Helping debug issues
- Automating repetitive tasks

You can use markdown formatting in your responses for better readability.

When suggesting shell commands for the user to execute:
1. Use ```shell-input code blocks (NOT bash/sh/shell) like this:
   ```shell-input
   command here
   ```
2. Each ```shell-input block represents ONE command option
3. Multiple ```shell-input blocks = alternative options for the user to choose from
4. For multi-line commands (like loops or scripts), put them in a SINGLE ```shell-input block
5. Always explain what each command does and why
6. Warn about potentially dangerous operations

Examples:

Single command:
```shell-input
ls -la
```

Alternative options:
```shell-input
git status
```
```shell-input
git diff
```

Multi-line command:
```shell-input
for file in *.txt; do
  echo "Processing $file"
  cat "$file"
done
```

You have access to:
- Recent terminal history from the active process
- The current working directory
- Exit codes from recent commands

Be concise but thorough. Prioritize practical solutions."#
        .to_string()
}

/// Format terminal context into a prompt
pub fn format_context(
  history: &[String],
  cwd: &PathBuf,
  last_exit_code: Option<i32>,
) -> String {
  let mut context = String::new();

  context.push_str("## Current Context\n\n");

  // Working directory
  context.push_str(&format!("**Working Directory:** `{}`\n\n", cwd.display()));

  // Last exit code
  if let Some(code) = last_exit_code {
    context.push_str(&format!("**Last Exit Code:** {}\n", code));
    if code != 0 {
      context.push_str("(Command failed)\n");
    }
    context.push('\n');
  }

  // Terminal history
  if !history.is_empty() {
    context.push_str("## Recent Terminal Output\n\n");
    context.push_str("```\n");

    // Include last 50 lines or all lines if fewer
    let start = history.len().saturating_sub(50);
    for line in &history[start..] {
      context.push_str(line);
      context.push('\n');
    }

    context.push_str("```\n");
  }

  context
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_system_prompt() {
    let prompt = system_prompt();
    assert!(prompt.contains("Termin.AI"));
    assert!(prompt.contains("bash"));
  }

  #[test]
  fn test_format_context() {
    let history = vec!["ls -la".to_string(), "cargo build".to_string()];
    let cwd = PathBuf::from("/home/user/project");
    let context = format_context(&history, &cwd, Some(0));

    assert!(context.contains("/home/user/project"));
    assert!(context.contains("ls -la"));
    assert!(context.contains("cargo build"));
    assert!(context.contains("Exit Code"));
  }

  #[test]
  fn test_format_context_with_error() {
    let history = vec!["failed command".to_string()];
    let cwd = PathBuf::from("/tmp");
    let context = format_context(&history, &cwd, Some(1));

    assert!(context.contains("Command failed"));
  }
}
