"""Configuration for LLM providers and models."""

import os
from dataclasses import dataclass
from enum import Enum


class Provider(str, Enum):
    """Supported LLM providers."""

    ANTHROPIC = "anthropic"
    OPENAI = "openai"
    GEMINI = "gemini"
    OLLAMA = "ollama"
    OPENROUTER = "openrouter"


@dataclass
class ProviderConfig:
    """Configuration for an LLM provider."""

    provider: Provider
    model: str
    api_key_env: str | None
    endpoint: str | None = None

    @classmethod
    def from_env(cls, provider: Provider | None = None) -> "ProviderConfig":
        """Create provider config from environment variables.

        Args:
            provider: The provider to configure. If None, reads from TERMINAI_PROVIDER env var.

        Returns:
            ProviderConfig instance

        Raises:
            ValueError: If required API key is not set or provider not specified
        """
        # Determine provider from environment if not specified
        if provider is None:
            provider_str = os.getenv("TERMINAI_PROVIDER")
            if not provider_str:
                raise ValueError(
                    "Provider not specified and TERMINAI_PROVIDER environment variable not set"
                )
            try:
                provider = Provider(provider_str.lower())
            except ValueError:
                raise ValueError(f"Unknown provider: {provider_str}")

        # Default models for each provider
        default_models = {
            Provider.ANTHROPIC: "claude-sonnet-4-5",
            Provider.OPENAI: "gpt-5.1",
            Provider.GEMINI: "gemini-2.5-pro",
            Provider.OLLAMA: "llama3",
            Provider.OPENROUTER: "google/gemma-3-27b-it:free",
        }

        # API key environment variables
        api_key_envs = {
            Provider.ANTHROPIC: "ANTHROPIC_API_KEY",
            Provider.OPENAI: "OPENAI_API_KEY",
            Provider.GEMINI: "GOOGLE_API_KEY",
            Provider.OLLAMA: None,  # Local server, no API key
            Provider.OPENROUTER: "OPENROUTER_API_KEY",
        }

        model = os.getenv(f"{provider.value.upper()}_MODEL", default_models[provider])
        api_key_env = api_key_envs[provider]

        # Verify API key is set if required
        if api_key_env and not os.getenv(api_key_env):
            raise ValueError(
                f"API key environment variable {api_key_env} not set for {provider.value}"
            )

        return cls(
            provider=provider,
            model=model,
            api_key_env=api_key_env,
        )

    def get_api_key(self) -> str | None:
        """Get the API key from environment."""
        if self.api_key_env:
            return os.getenv(self.api_key_env)
        return None
