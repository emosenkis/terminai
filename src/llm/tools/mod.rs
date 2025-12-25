// TERMIN.AI: Tools for AI agent

pub mod grep_files;
pub mod read_file;
pub mod read_scrollback;
pub mod suggest_command;

pub use grep_files::{GrepFilesArgs, GrepFilesTool};
pub use read_file::{ReadFileArgs, ReadFileTool};
pub use read_scrollback::{ReadScrollbackArgs, ReadScrollbackTool};
pub use suggest_command::{
  SuggestCommandArgs, SuggestCommandTool, SuggestedCommand,
};
