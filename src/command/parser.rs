use regex::Regex;

/// Extract bash/shell commands from markdown code blocks
pub struct CommandParser {
  code_block_regex: Regex,
}

impl CommandParser {
  pub fn new() -> Self {
    // Match code blocks with bash, sh, or shell language tags
    let code_block_regex = Regex::new(r"```(?:bash|sh|shell)\n([\s\S]*?)```")
      .expect("Invalid regex");

    Self { code_block_regex }
  }

  /// Extract all commands from markdown text
  pub fn extract_commands(&self, markdown: &str) -> Vec<String> {
    self
      .code_block_regex
      .captures_iter(markdown)
      .map(|cap| cap.get(1).unwrap().as_str().trim().to_string())
      .filter(|cmd| !cmd.is_empty())
      .collect()
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
```bash
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
```bash
pwd
```

Then list files:
```sh
ls -la
```

Finally, check disk usage:
```shell
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
```bash
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
```bash
```"#;

    let commands = parser.extract_commands(markdown);
    assert_eq!(commands.len(), 0);
  }

  #[test]
  fn test_first_command() {
    let parser = CommandParser::new();
    let markdown = r#"
```bash
ls
```
More text
```bash
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
```bash
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
}
