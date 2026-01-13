"""Integration tests for Claude SDK adapter with AG-UI protocol."""

import os
from unittest.mock import AsyncMock, patch

import pytest
from ag_ui.core import EventType, RunAgentInput, UserMessage

from terminai_agent.agent import TerminalContext, create_agent_adapter
from terminai_agent.config import Provider, ProviderConfig


@pytest.fixture
def mock_env():
    """Mock environment with API key."""
    with patch.dict(os.environ, {"ANTHROPIC_API_KEY": "test-key-123"}):
        yield


@pytest.fixture
def terminal_context():
    """Sample terminal context."""
    return TerminalContext(
        cwd="/home/user/project",
        history_lines=["$ ls", "file1.txt  file2.txt"],
        last_exit_code=0,
        os_info="Linux",
        shell="bash",
    )


@pytest.fixture
def provider_config():
    """Provider config for Anthropic."""
    return ProviderConfig(
        provider=Provider.ANTHROPIC,
        model="claude-sonnet-4-5",
        api_key_env="ANTHROPIC_API_KEY",
    )


def test_adapter_creation_with_context(mock_env, provider_config, terminal_context):
    """Test that adapter is created with terminal context in system prompt."""
    adapter = create_agent_adapter(provider_config, terminal_context)

    assert adapter is not None
    assert hasattr(adapter, "run")
    assert adapter.api_key == "test-key-123"
    assert adapter.cwd == "/home/user/project"

    # Verify system prompt construction by checking adapter's internal state
    # The adapter stores the system prompt but doesn't expose it directly
    # We can verify it was configured correctly by checking the kwargs
    assert adapter._options_kwargs.get("cwd") == "/home/user/project"
    assert adapter._options_kwargs.get("allowed_tools") == ["Read", "Grep", "Bash"]
    assert adapter._options_kwargs.get("permission_mode") == "acceptEdits"


def test_adapter_system_prompt_includes_context(mock_env, provider_config, terminal_context):
    """Test that system prompt includes terminal context information."""
    adapter = create_agent_adapter(provider_config, terminal_context)

    # The system prompt should be in the options_kwargs
    system_prompt = adapter._options_kwargs.get("system_prompt", "")

    # Verify key context elements are in the system prompt
    assert "/home/user/project" in system_prompt
    assert "Linux" in system_prompt
    assert "bash" in system_prompt
    assert "Last Exit Code:** 0" in system_prompt  # Check markdown formatted version
    assert "$ ls" in system_prompt


def test_adapter_with_openai_provider_fallback(mock_env, terminal_context):
    """Test that OpenAI provider falls back to Anthropic with warning."""
    config = ProviderConfig(
        provider=Provider.OPENAI,
        model="gpt-4",
        api_key_env="OPENAI_API_KEY",
    )

    # Should create adapter but use ANTHROPIC_API_KEY
    with patch("terminai_agent.agent.logger") as mock_logger:
        adapter = create_agent_adapter(config, terminal_context)

        # Verify warning was logged
        mock_logger.warning.assert_called()
        warning_msg = mock_logger.warning.call_args[0][0]
        assert "Claude SDK only supports Anthropic" in warning_msg
        assert "openai" in warning_msg.lower()

    # Adapter should still be created with fallback
    assert adapter is not None
    assert adapter.api_key == "test-key-123"  # Falls back to ANTHROPIC_API_KEY


def test_adapter_has_run_method(mock_env, provider_config, terminal_context):
    """Test that adapter has the run method for AG-UI protocol."""
    adapter = create_agent_adapter(provider_config, terminal_context)

    # Verify the adapter has the run method
    assert hasattr(adapter, "run")
    assert callable(adapter.run)

    # Verify it's an async method by checking if it's a coroutine function
    import inspect
    assert inspect.iscoroutinefunction(adapter.run) or inspect.isasyncgenfunction(adapter.run)


def test_adapter_allowed_tools_configured(mock_env, provider_config, terminal_context):
    """Test that adapter has the correct tools configured."""
    adapter = create_agent_adapter(provider_config, terminal_context)

    # Verify the allowed tools are configured
    allowed_tools = adapter._options_kwargs.get("allowed_tools", [])
    assert "Read" in allowed_tools
    assert "Grep" in allowed_tools
    assert "Bash" in allowed_tools
    assert len(allowed_tools) == 3


def test_adapter_permission_mode(mock_env, provider_config, terminal_context):
    """Test that adapter uses acceptEdits permission mode."""
    adapter = create_agent_adapter(provider_config, terminal_context)

    # Verify permission mode
    permission_mode = adapter._options_kwargs.get("permission_mode")
    assert permission_mode == "acceptEdits"
