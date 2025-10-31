// TERMIN.AI: LLM client module for AI assistance

pub mod client;
pub mod prompts;
pub mod providers;

pub use client::{LLMClient, TerminalContext};
pub use providers::Provider;
