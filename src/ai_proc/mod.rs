// TERMIN.AI: AI chat process module

pub mod chat_process;
pub mod context;
pub mod ui;

// Re-exports will be used once AI integration is complete
#[allow(unused_imports)]
pub use chat_process::{
  AIChatProcess, ConversationEntry, Message, MessageRole, PendingCommand,
};
#[allow(unused_imports)]
pub use context::ContextExtractor;
#[allow(unused_imports)]
pub use ui::AIChatUI;
