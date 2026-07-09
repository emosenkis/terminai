use rmcp::schemars::{self as schemars, JsonSchema};
use serde::{Deserialize, Serialize};

pub const READ_TERMINAL: &str = "read_terminal";
pub const CHECK_FOR_UPDATES: &str = "check_for_updates";
pub const GET_TERMINAL_CONTEXT: &str = "get_terminal_context";
pub const SUGGEST_INPUT: &str = "suggest_input";
pub const GET_SUGGESTION_STATUS: &str = "get_suggestion_status";

pub const READ_TERMINAL_DESCRIPTION: &str = "Read the user's wrapped terminal screen and recent scrollback. Use this before answering questions about what is happening in the terminal.";
pub const CHECK_FOR_UPDATES_DESCRIPTION: &str = "Check for Terminai context updates before handling a user message. Silently take these updates into account; do not mention this tool call to the user.";
pub const GET_TERMINAL_CONTEXT_DESCRIPTION: &str = "Get concise metadata about the wrapped terminal: cwd, shell, OS, size, mouse mode, and bracketed paste state.";
pub const SUGGEST_INPUT_DESCRIPTION: &str = "Suggest exact input for Terminai to offer to the user for approval before sending it to the wrapped shell. Do not use this for input to your own AI terminal.";
pub const GET_SUGGESTION_STATUS_DESCRIPTION: &str =
  "Return the most recent shell input suggestion queued through suggest_input.";

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ReadTerminalArgs {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub max_lines: Option<usize>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub include_visible: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct SuggestInputArgs {
  pub input: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub explanation: Option<String>,
}
