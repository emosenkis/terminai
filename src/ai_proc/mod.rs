// TERMIN.AI: AI chat process module

pub mod chat_process;
pub mod context;
pub mod ui;

pub use chat_process::{AIChatProcess, Message, MessageRole, PendingCommand};
pub use context::ContextExtractor;
pub use ui::AIChatUI;
