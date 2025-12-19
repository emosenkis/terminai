use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::{Arc, Mutex};

#[derive(Deserialize)]
pub struct SuggestCommandArgs {
  /// The command to suggest for execution
  command: String,
  /// Explanation of what the command does
  explanation: String,
  /// Whether this command contains raw escape sequences (ctrl-c, page-up, etc.)
  #[serde(default)]
  raw: bool,
}

#[derive(Debug, Clone)]
pub struct SuggestedCommand {
  pub command: String,
  pub explanation: String,
  pub raw: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum SuggestCommandError {
  #[error("Failed to store suggested command")]
  StorageError,
  #[error("Empty command provided")]
  EmptyCommand,
}

/// Tool for suggesting commands to run in the terminal
/// This replaces the old ```shell-input markdown code blocks approach
pub struct SuggestCommandTool {
  /// Storage for suggested commands
  suggested_commands: Arc<Mutex<Vec<SuggestedCommand>>>,
}

impl SuggestCommandTool {
  pub fn new(suggested_commands: Arc<Mutex<Vec<SuggestedCommand>>>) -> Self {
    Self { suggested_commands }
  }
}

impl Tool for SuggestCommandTool {
  const NAME: &'static str = "suggest_command";

  type Args = SuggestCommandArgs;
  type Output = String;
  type Error = SuggestCommandError;

  async fn definition(&self, _prompt: String) -> ToolDefinition {
    ToolDefinition {
      name: "suggest_command".to_string(),
      description: "Suggest a command for the user to execute in the terminal. The command will be presented to the user for approval before execution.".to_string(),
      parameters: json!({
        "type": "object",
        "properties": {
          "command": {
            "type": "string",
            "description": "The shell command to suggest (e.g., 'ls -la', 'git status')"
          },
          "explanation": {
            "type": "string",
            "description": "A clear explanation of what this command does and why you're suggesting it"
          },
          "raw": {
            "type": "boolean",
            "description": "Set to true if the command contains raw escape sequences (e.g., Ctrl-C, arrow keys). Default is false.",
            "default": false
          }
        },
        "required": ["command", "explanation"]
      }),
    }
  }

  async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
    // Validate command is not empty
    if args.command.trim().is_empty() {
      return Err(SuggestCommandError::EmptyCommand);
    }

    // Store the suggested command
    let suggested = SuggestedCommand {
      command: args.command.clone(),
      explanation: args.explanation.clone(),
      raw: args.raw,
    };

    self
      .suggested_commands
      .lock()
      .map_err(|_| SuggestCommandError::StorageError)?
      .push(suggested);

    // Return confirmation message
    let raw_indicator = if args.raw {
      " (contains escape sequences)"
    } else {
      ""
    };
    Ok(format!(
      "Command suggested{}: `{}`\n\nExplanation: {}",
      raw_indicator, args.command, args.explanation
    ))
  }
}
