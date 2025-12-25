"""Integration tests for LLM client that can be run independently."""

from typing import Any

import pytest

from terminai_llm.client import LLMClient, SuggestedCommand, TerminalContext


@pytest.fixture
def mock_env(monkeypatch: pytest.MonkeyPatch) -> None:
    """Set up mock environment variables."""
    monkeypatch.setenv("ANTHROPIC_API_KEY", "test-key")
    monkeypatch.setenv("OPENAI_API_KEY", "test-key")
    monkeypatch.setenv("GOOGLE_API_KEY", "test-key")
    monkeypatch.setenv("OLLAMA_BASE_URL", "http://localhost:11434")


def test_client_can_be_instantiated(mock_env: None) -> None:
    """Test that client can be created without errors."""
    client = LLMClient(provider="anthropic")
    assert client.provider == "anthropic"
    assert client.model_name == "claude-sonnet-4-5"


def test_client_with_different_providers(mock_env: None) -> None:
    """Test client creation with different providers."""
    providers_models = [
        ("anthropic", "claude-sonnet-4-5"),
        ("openai", "gpt-4"),
        ("google-vertex", "gemini-2.0-flash-exp"),
    ]

    for provider, expected_model in providers_models:
        client = LLMClient(provider=provider)
        assert client.provider == provider
        assert client.model_name == expected_model


def test_terminal_context_conversion(mock_env: None) -> None:
    """Test that terminal context is properly formatted."""
    client = LLMClient(provider="anthropic")

    ctx = TerminalContext(
        cwd="/home/user/project",
        history_lines=["$ ls -la", "total 42", "$ pwd", "/home/user/project"],
        last_exit_code=0,
    )

    formatted = client._format_context(ctx)
    assert "/home/user/project" in formatted
    assert "$ ls -la" in formatted
    assert "✓" in formatted  # Success indicator for exit code 0


def test_suggested_commands_storage(mock_env: None) -> None:
    """Test that suggested commands can be stored and retrieved."""
    client = LLMClient(provider="anthropic")

    # Initially empty
    commands = client.take_suggested_commands()
    assert len(commands) == 0

    # Add a command directly to internal storage
    client._suggested_commands.append(
        SuggestedCommand(
            command="ls -la",
            explanation="List all files including hidden ones",
            raw=False,
        )
    )

    # Retrieve commands
    commands = client.take_suggested_commands()
    assert len(commands) == 1
    assert commands[0]["command"] == "ls -la"
    assert commands[0]["explanation"] == "List all files including hidden ones"
    assert commands[0]["raw"] is False

    # Verify they're cleared
    commands = client.take_suggested_commands()
    assert len(commands) == 0


@pytest.mark.asyncio
async def test_streaming_interface_exists(mock_env: None) -> None:
    """Test that streaming interface is available (even if not fully implemented)."""
    client = LLMClient(provider="anthropic")

    context: dict[str, Any] = {
        "cwd": "/tmp",
        "history_lines": [],
        "last_exit_code": None,
    }
    history: list[dict[str, str]] = []

    # Just verify the method exists and returns something with __anext__
    stream = client.send_message_stream("test", context, history)
    assert hasattr(stream, "__anext__")


def test_message_history_conversion(mock_env: None) -> None:
    """Test that message history is properly converted."""
    client = LLMClient(provider="anthropic")

    history = [
        {"role": "user", "content": "Hello"},
        {"role": "assistant", "content": "Hi there!"},
        {"role": "user", "content": "How are you?"},
    ]

    converted = client._convert_history(history)
    assert len(converted) == 3
    assert all("role" in msg and "content" in msg for msg in converted)


if __name__ == "__main__":
    # Allow running tests directly
    pytest.main([__file__, "-v"])
