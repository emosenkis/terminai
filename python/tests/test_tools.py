"""Unit tests for tool registry."""

import pytest

from terminai_llm.tools import ToolRegistry


def test_tool_registry_initialization() -> None:
    """Test ToolRegistry initialization."""
    registry = ToolRegistry()
    assert len(registry._tools) == 0
    assert len(registry._callbacks) == 0


def test_register_tool_basic() -> None:
    """Test basic tool registration."""
    registry = ToolRegistry()
    registry.register_tool(
        name="test_tool",
        description="A test tool",
        parameters={
            "type": "object",
            "properties": {"arg": {"type": "string"}},
        },
    )
    assert "test_tool" in registry._tools
    assert registry._tools["test_tool"]["type"] == "function"


def test_register_tool_with_callback() -> None:
    """Test tool registration with callback."""

    def test_callback(args: dict[str, str]) -> str:
        return f"Called with {args}"

    registry = ToolRegistry()
    registry.register_tool(
        name="test_tool",
        description="A test tool",
        parameters={},
        callback=test_callback,
    )
    assert "test_tool" in registry._tools
    assert "test_tool" in registry._callbacks


def test_register_callback_for_existing_tool() -> None:
    """Test registering callback for existing tool."""

    def callback(args: dict[str, str]) -> str:
        return "result"

    registry = ToolRegistry()
    registry.register_tool(
        name="test_tool",
        description="Test",
        parameters={},
    )
    registry.register_callback("test_tool", callback)
    assert "test_tool" in registry._callbacks


def test_register_callback_for_nonexistent_tool() -> None:
    """Test that registering callback for nonexistent tool raises error."""
    registry = ToolRegistry()

    def callback(args: dict[str, str]) -> str:
        return "result"

    with pytest.raises(ValueError, match="Tool not registered"):
        registry.register_callback("nonexistent", callback)


def test_get_tool_definitions() -> None:
    """Test getting tool definitions."""
    registry = ToolRegistry()
    registry.register_tool(
        name="tool1",
        description="First tool",
        parameters={},
    )
    registry.register_tool(
        name="tool2",
        description="Second tool",
        parameters={},
    )

    definitions = registry.get_tool_definitions()
    assert len(definitions) == 2
    assert all(d["type"] == "function" for d in definitions)


@pytest.mark.asyncio
async def test_execute_tool_sync_callback() -> None:
    """Test executing tool with synchronous callback."""

    def sync_callback(args: dict[str, str]) -> str:
        return f"Processed: {args.get('input', '')}"

    registry = ToolRegistry()
    registry.register_tool(
        name="sync_tool",
        description="Sync tool",
        parameters={},
        callback=sync_callback,
    )

    result = await registry.execute_tool("sync_tool", {"input": "test"})
    assert result == "Processed: test"


@pytest.mark.asyncio
async def test_execute_tool_async_callback() -> None:
    """Test executing tool with async callback."""

    async def async_callback(args: dict[str, str]) -> str:
        return f"Async: {args.get('input', '')}"

    registry = ToolRegistry()
    registry.register_tool(
        name="async_tool",
        description="Async tool",
        parameters={},
        callback=async_callback,
    )

    result = await registry.execute_tool("async_tool", {"input": "test"})
    assert result == "Async: test"


@pytest.mark.asyncio
async def test_execute_tool_no_callback() -> None:
    """Test executing tool with no callback raises error."""
    registry = ToolRegistry()
    registry.register_tool(
        name="no_callback",
        description="No callback",
        parameters={},
    )

    with pytest.raises(ValueError, match="No callback registered"):
        await registry.execute_tool("no_callback", {})
