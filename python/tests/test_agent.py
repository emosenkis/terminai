"""Tests for the Termin.AI agent."""

import os
from unittest.mock import patch

import pytest

from terminai_agent.agent import TerminalContext, create_agent
from terminai_agent.config import Provider


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


@pytest.mark.asyncio
async def test_create_agent_anthropic(mock_env):
    """Test creating an agent with Anthropic provider."""
    agent = await create_agent(Provider.ANTHROPIC)
    assert agent is not None
    assert agent.provider_config.provider == Provider.ANTHROPIC


@pytest.mark.asyncio
async def test_create_agent_from_string(mock_env):
    """Test creating an agent from provider string."""
    agent = await create_agent("anthropic")
    assert agent.provider_config.provider == Provider.ANTHROPIC


@pytest.mark.asyncio
async def test_create_agent_missing_api_key():
    """Test that missing API key raises error."""
    with pytest.raises(ValueError, match="API key environment variable.*not set"):
        await create_agent(Provider.ANTHROPIC)


def test_build_message_with_context(mock_env, terminal_context):
    """Test context formatting in messages."""
    from terminai_agent.agent import TerminAIAgent
    from terminai_agent.config import ProviderConfig

    config = ProviderConfig.from_env(Provider.ANTHROPIC)
    agent = TerminAIAgent(config)

    message = agent._build_message_with_context("Help me debug this", terminal_context)

    assert "/home/user/project" in message
    assert "Last Exit Code" in message and "0" in message
    assert "Recent Terminal Output" in message
    assert "Hello, world!" in message
    assert "Help me debug this" in message
