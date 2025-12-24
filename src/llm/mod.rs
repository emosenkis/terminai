// TERMIN.AI: LLM client module for AI assistance

pub mod client;
pub mod prompts;
pub mod providers;
pub mod tools;

// Python bridge (experimental)
#[cfg(feature = "python-llm")]
pub mod python_bridge;

pub use client::{ChatMessage, LLMClient, TerminalContext};
pub use providers::Provider;

#[cfg(feature = "python-llm")]
pub use python_bridge::PythonLLMBridge;
