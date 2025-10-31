use std::path::PathBuf;

use crate::llm::TerminalContext;

/// Extract terminal context from process views
pub struct ContextExtractor {
    max_history_lines: usize,
}

impl ContextExtractor {
    pub fn new(max_history_lines: usize) -> Self {
        Self { max_history_lines }
    }

    /// Extract context from terminal output
    /// TODO: This will be integrated with mprocs' ProcView to extract actual terminal history
    pub fn extract_context(
        &self,
        _process_id: Option<usize>,
        cwd: PathBuf,
    ) -> TerminalContext {
        // Placeholder implementation
        // In the full implementation, this will:
        // 1. Get the target process's ProcView
        // 2. Extract scrollback buffer lines
        // 3. Get the last exit code
        // 4. Filter sensitive information

        TerminalContext::empty(cwd)
    }

    /// Get working directory from environment
    pub fn get_cwd() -> PathBuf {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))
    }
}

impl Default for ContextExtractor {
    fn default() -> Self {
        Self::new(100)
    }
}
