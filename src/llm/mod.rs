// TERMIN.AI: LLM client module for AI assistance

pub mod adapter;
pub mod client;
pub mod prompts;
pub mod providers;
pub mod tools;

// Python bridge (experimental)
#[cfg(feature = "python-llm")]
pub mod python_bridge;

pub use client::{ChatMessage, LLMClient, TerminalContext};
pub use providers::Provider;

// Adapter for switching between Rig and Python backends
pub use adapter::LLMClientAdapter;

#[cfg(feature = "python-llm")]
pub use python_bridge::PythonLLMBridge;
