"""Tests for the Termin.AI agent."""

import os
from unittest.mock import patch

import pytest

from terminai_agent.agent import TerminalContext, create_agent_adapter
from terminai_agent.config import Provider, ProviderConfig


@pytest.fixture
def mock_env():
    """Mock environment variables for testing."""
    with patch.dict(
        os.environ,
        {
            "ANTHROPIC_API_KEY": "test-key-123",
            "OPENAI_API_KEY": "test-key-456",
            "GOOGLE_API_KEY": "test-key-789",
        },
    ):
        yield


@pytest.fixture
def terminal_context():
    """Sample terminal context for testing."""
    return TerminalContext(
        history_lines=[
            "$ ls",
            "file1.txt  file2.txt",
            "$ cat file1.txt",
            "Hello, world!",
        ],
        cwd="/home/user/project",
        last_exit_code=0,
    )


def test_terminal_context_creation(terminal_context):
    """Test TerminalContext model creation."""
    assert terminal_context.cwd == "/home/user/project"
    assert terminal_context.last_exit_code == 0
    assert len(terminal_context.history_lines) == 4


def test_terminal_context_optional_fields():
    """Test TerminalContext with minimal fields."""
    context = TerminalContext(
        history_lines=[],
        cwd="/tmp",
    )
    assert context.last_exit_code is None


def test_create_agent_adapter_anthropic(mock_env, terminal_context):
    """Test creating an agent adapter with Anthropic provider."""
    config = ProviderConfig.from_env(Provider.ANTHROPIC)
    adapter = create_agent_adapter(config, terminal_context)
    assert adapter is not None
    # The adapter should have the Claude SDK configuration
    assert hasattr(adapter, "run")


def test_create_agent_adapter_no_context(mock_env):
    """Test creating an agent adapter without terminal context."""
    config = ProviderConfig.from_env(Provider.ANTHROPIC)
    adapter = create_agent_adapter(config, None)
    assert adapter is not None


def test_create_agent_adapter_missing_api_key():
    """Test that missing API key is handled (warning logged)."""
    # This should create the adapter but log a warning
    # The actual API call will fail, but adapter creation should work
    config = ProviderConfig(
        provider=Provider.ANTHROPIC,
        model="claude-sonnet-4-5",
        api_key_env="ANTHROPIC_API_KEY",
    )
    # This creates the adapter but doesn't make API calls yet
    adapter = create_agent_adapter(config, None)
    assert adapter is not None
