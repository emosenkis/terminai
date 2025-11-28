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
      Provider::OpenRouter => "anthropic/claude-3-5-sonnet",
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
