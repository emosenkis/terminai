"""Unit tests for LLM client."""

import os
from typing import Any

import pytest

from terminai_llm.client import LLMClient, SuggestedCommand, TerminalContext


@pytest.fixture(autouse=True)
def set_dummy_api_keys() -> None:
    """Set dummy API keys and config for testing."""
    os.environ["ANTHROPIC_API_KEY"] = "test-key"
    os.environ["OPENAI_API_KEY"] = "test-key"
    os.environ["GOOGLE_API_KEY"] = "test-key"
    os.environ["OLLAMA_BASE_URL"] = "http://localhost:11434"


def test_suggested_command_creation() -> None:
    """Test SuggestedCommand model creation."""
    cmd = SuggestedCommand(
        command="ls -la",
        explanation="List all files in long format",
    )
    assert cmd.command == "ls -la"
    assert cmd.explanation == "List all files in long format"
    assert cmd.raw is False


def test_terminal_context_creation() -> None:
    """Test TerminalContext model creation."""
    ctx = TerminalContext(
        cwd="/home/user",
        history_lines=["$ ls", "file.txt", "$ pwd"],
        last_exit_code=0,
    )
    assert ctx.cwd == "/home/user"
    assert len(ctx.history_lines) == 3
    assert ctx.last_exit_code == 0


def test_terminal_context_defaults() -> None:
    """Test TerminalContext defaults."""
    ctx = TerminalContext(cwd="/tmp")
    assert ctx.cwd == "/tmp"
    assert ctx.history_lines == []
    assert ctx.last_exit_code is None


def test_client_initialization() -> None:
    """Test LLMClient initialization."""
    client = LLMClient(provider="anthropic", model="claude-sonnet-4-5")
    assert client.provider == "anthropic"
    assert client.model_name == "claude-sonnet-4-5"
    assert len(client._suggested_commands) == 0


def test_client_default_models() -> None:
    """Test default model selection for different providers."""
    anthropic_client = LLMClient(provider="anthropic")
    assert anthropic_client.model_name == "claude-sonnet-4-5"

    openai_client = LLMClient(provider="openai")
    assert openai_client.model_name == "gpt-4"

    google_client = LLMClient(provider="google-vertex")
    assert google_client.model_name == "gemini-2.0-flash-exp"

    ollama_client = LLMClient(provider="ollama")
    assert ollama_client.model_name == "llama3"


def test_format_context_basic() -> None:
    """Test terminal context formatting."""
    client = LLMClient(provider="anthropic")
    ctx = TerminalContext(cwd="/home/user")
    formatted = client._format_context(ctx)
    assert "📂 Current directory: /home/user" in formatted


def test_format_context_with_exit_code() -> None:
    """Test context formatting with exit code."""
    client = LLMClient(provider="anthropic")
    ctx = TerminalContext(cwd="/tmp", last_exit_code=0)
    formatted = client._format_context(ctx)
    assert "✓ Last exit code: 0" in formatted

    ctx_error = TerminalContext(cwd="/tmp", last_exit_code=1)
    formatted_error = client._format_context(ctx_error)
    assert "✗ Last exit code: 1" in formatted_error


def test_format_context_with_history() -> None:
    """Test context formatting with history."""
    client = LLMClient(provider="anthropic")
    ctx = TerminalContext(
        cwd="/tmp",
        history_lines=["$ ls", "file.txt", "$ pwd", "/tmp"],
    )
    formatted = client._format_context(ctx)
    assert "📜 Recent terminal output:" in formatted
    assert "$ ls" in formatted
    assert "file.txt" in formatted


def test_convert_history() -> None:
    """Test message history conversion."""
    client = LLMClient(provider="anthropic")
    history = [
        {"role": "user", "content": "Hello"},
        {"role": "assistant", "content": "Hi there!"},
    ]
    converted = client._convert_history(history)
    assert len(converted) == 2
    assert converted[0]["role"] == "user"
    assert converted[0]["content"] == "Hello"


def test_take_suggested_commands_empty() -> None:
    """Test taking suggested commands when none exist."""
    client = LLMClient(provider="anthropic")
    commands = client.take_suggested_commands()
    assert commands == []


def test_take_suggested_commands_clears() -> None:
    """Test that taking commands clears the internal list."""
    client = LLMClient(provider="anthropic")
    client._suggested_commands.append(
        SuggestedCommand(command="ls", explanation="List files")
    )
    commands = client.take_suggested_commands()
    assert len(commands) == 1
    assert len(client._suggested_commands) == 0

    commands_again = client.take_suggested_commands()
    assert commands_again == []


@pytest.mark.asyncio
async def test_send_message_stream_basic() -> None:
    """Test basic streaming functionality structure.

    Note: This test doesn't make actual API calls - it just verifies
    the interface structure. Full integration testing requires API keys.
    """
    client = LLMClient(provider="anthropic")
    context: dict[str, Any] = {"cwd": "/tmp", "history_lines": [], "last_exit_code": None}
    history: list[dict[str, str]] = []

    # We can't actually test streaming without API keys, but we can verify
    # the method exists and returns an async iterator
    stream = client.send_message_stream("test", context, history)
    assert hasattr(stream, "__anext__")


def test_system_prompt_content() -> None:
    """Test that system prompt contains key instructions."""
    client = LLMClient(provider="anthropic")
    prompt = client._get_system_prompt()
    assert "terminal emulator" in prompt.lower()
    assert "suggest_command" in prompt
    assert "current working directory" in prompt.lower()
