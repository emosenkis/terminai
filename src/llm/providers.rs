//! Provider enum for LLM providers
//!
//! NOTE: This enum is duplicated in Python (config.py) because:
//! - Rust: Pre-flight validation (check API keys before spawning subprocess)
//! - Python: Actual provider selection and configuration
//!
//! The duplication is intentional - each side needs its own copy for
//! its specific purpose. Keep in sync when adding new providers!

use serde::{Deserialize, Serialize};

/// LLM provider types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
  Anthropic,
  OpenAI,
  Gemini,
  Ollama,
  #[serde(rename = "openrouter")]
  OpenRouter,
}

impl Provider {
  /// Get the default model for this provider
  pub fn default_model(&self) -> &str {
    match self {
      Provider::Anthropic => "claude-sonnet-4-5",
      Provider::OpenAI => "gpt-5.1",
      Provider::Gemini => "gemini-2.5-pro",
      Provider::Ollama => "llama3",
      Provider::OpenRouter => "google/gemma-3-27b-it:free",
    }
  }

  /// Get the provider name for use with rig's DynClient
  pub fn provider_name(&self) -> &str {
    match self {
      Provider::Anthropic => "anthropic",
      Provider::OpenAI => "openai",
      Provider::Gemini => "gemini",
      Provider::Ollama => "ollama",
      Provider::OpenRouter => "openrouter",
    }
  }

  /// Get the environment variable name for the API key
  pub fn api_key_env(&self) -> Option<&str> {
    match self {
      Provider::Anthropic => Some("ANTHROPIC_API_KEY"),
      Provider::OpenAI => Some("OPENAI_API_KEY"),
      Provider::Gemini => Some("GOOGLE_API_KEY"),
      Provider::Ollama => None, // Ollama runs locally
      Provider::OpenRouter => Some("OPENROUTER_API_KEY"),
    }
  }
}

impl std::fmt::Display for Provider {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Provider::Anthropic => write!(f, "anthropic"),
      Provider::OpenAI => write!(f, "openai"),
      Provider::Gemini => write!(f, "gemini"),
      Provider::Ollama => write!(f, "ollama"),
      Provider::OpenRouter => write!(f, "openrouter"),
    }
  }
}

impl std::str::FromStr for Provider {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s.to_lowercase().as_str() {
      "anthropic" | "claude" => Ok(Provider::Anthropic),
      "openai" | "gpt" => Ok(Provider::OpenAI),
      "gemini" | "google" => Ok(Provider::Gemini),
      "ollama" => Ok(Provider::Ollama),
      "openrouter" => Ok(Provider::OpenRouter),
      _ => Err(format!("Unknown provider: {}", s)),
    }
  }
}
