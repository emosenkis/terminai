use regex::Regex;

/// Extract shell commands from markdown code blocks
pub struct CommandParser {
  code_block_regex: Regex,
}

impl CommandParser {
  pub fn new() -> Self {
    // Match code blocks with shell-input language tag
    // Allow optional whitespace after language tag and handle both \n and \r\n
    // Case-insensitive to handle variations like Shell-Input or SHELL-INPUT
    let code_block_regex =
      Regex::new(r"(?i)```shell-input\s*\r?\n([\s\S]*?)```")
        .expect("Invalid regex");

    Self { code_block_regex }
  }

  /// Extract all commands from markdown text
  pub fn extract_commands(&self, markdown: &str) -> Vec<String> {
    log::debug!(
      "Parsing markdown for commands (length: {} chars)",
      markdown.len()
    );
    log::debug!(
      "Markdown preview (first 500 chars): {:?}",
      &markdown.chars().take(500).collect::<String>()
    );

    let commands: Vec<String> = self
      .code_block_regex
      .captures_iter(markdown)
      .map(|cap| cap.get(1).unwrap().as_str().trim().to_string())
      .filter(|cmd| !cmd.is_empty())
      .collect();

    log::debug!("Found {} commands", commands.len());
    for (i, cmd) in commands.iter().enumerate() {
      log::debug!("Command {}: {:?}", i, cmd);
    }

    commands
  }

  /// Extract the first command from markdown text
  pub fn extract_first_command(&self, markdown: &str) -> Option<String> {
    self.extract_commands(markdown).into_iter().next()
  }
}

impl Default for CommandParser {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_extract_single_command() {
    let parser = CommandParser::new();
    let markdown = r#"Here's how to list files:
```shell-input
ls -la
```
That's it!"#;

    let commands = parser.extract_commands(markdown);
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0], "ls -la");
  }

  #[test]
  fn test_extract_multiple_commands() {
    let parser = CommandParser::new();
    let markdown = r#"First, check the directory:
```shell-input
pwd
```

Then list files:
```shell-input
ls -la
```

Finally, check disk usage:
```shell-input
df -h
```"#;

    let commands = parser.extract_commands(markdown);
    assert_eq!(commands.len(), 3);
    assert_eq!(commands[0], "pwd");
    assert_eq!(commands[1], "ls -la");
    assert_eq!(commands[2], "df -h");
  }

  #[test]
  fn test_multiline_command() {
    let parser = CommandParser::new();
    let markdown = r#"Here's a complex command:
```shell-input
for file in *.txt; do
    echo "Processing $file"
    cat "$file"
done
```"#;

    let commands = parser.extract_commands(markdown);
    assert_eq!(commands.len(), 1);
    assert!(commands[0].contains("for file"));
    assert!(commands[0].contains("done"));
  }

  #[test]
  fn test_no_commands() {
    let parser = CommandParser::new();
    let markdown = "This is just text with no code blocks.";

    let commands = parser.extract_commands(markdown);
    assert_eq!(commands.len(), 0);
  }

  #[test]
  fn test_empty_code_block() {
    let parser = CommandParser::new();
    let markdown = r#"Empty block:
```shell-input
```"#;

    let commands = parser.extract_commands(markdown);
    assert_eq!(commands.len(), 0);
  }

  #[test]
  fn test_first_command() {
    let parser = CommandParser::new();
    let markdown = r#"
```shell-input
ls
```
More text
```shell-input
pwd
```"#;

    let first = parser.extract_first_command(markdown);
    assert_eq!(first, Some("ls".to_string()));
  }

  #[test]
  fn test_code_block_with_other_languages() {
    let parser = CommandParser::new();
    let markdown = r#"
Python code:
```python
print("hello")
```

Shell code:
```shell-input
echo "hello"
```

JavaScript code:
```javascript
console.log("hello");
```"#;

    let commands = parser.extract_commands(markdown);
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0], "echo \"hello\"");
  }

  #[test]
  fn test_code_block_with_whitespace() {
    let parser = CommandParser::new();
    // Test with whitespace after shell-input
    let markdown = r#"Command with space:
```shell-input 
ls -la
```"#;

    let commands = parser.extract_commands(markdown);
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0], "ls -la");
  }

  #[test]
  fn test_code_block_case_insensitive() {
    let parser = CommandParser::new();
    // Test case-insensitive matching
    let markdown = r#"Command with different case:
```Shell-Input
pwd
```"#;

    let commands = parser.extract_commands(markdown);
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0], "pwd");
  }
}
