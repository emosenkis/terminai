"""Tool registry for LLM tools that can call back to Rust."""

import asyncio
from collections.abc import Callable
from typing import Any


class ToolRegistry:
    """Registry for LLM tools that can call back to Rust."""

    def __init__(self) -> None:
        """Initialize the tool registry."""
        self._tools: dict[str, dict[str, Any]] = {}
        self._callbacks: dict[str, Callable[..., Any]] = {}

    def register_tool(
        self,
        name: str,
        description: str,
        parameters: dict[str, Any],
        callback: Callable[..., Any] | None = None,
    ) -> None:
        """Register a new tool.

        Args:
            name: Tool name
            description: Tool description for LLM
            parameters: JSON schema for tool parameters
            callback: Optional callback function (sync or async)
        """
        self._tools[name] = {
            "type": "function",
            "function": {
                "name": name,
                "description": description,
                "parameters": parameters,
            },
        }
        if callback:
            self._callbacks[name] = callback

    def register_callback(self, name: str, callback: Callable[..., Any]) -> None:
        """Register or update a callback for an existing tool.

        Args:
            name: Tool name
            callback: Callback function (sync or async)
        """
        if name not in self._tools:
            raise ValueError(f"Tool not registered: {name}")
        self._callbacks[name] = callback

    def get_tool_definitions(self) -> list[dict[str, Any]]:
        """Get all tool definitions for LLM API.

        Returns:
            List of tool definition dicts
        """
        return list(self._tools.values())

    async def execute_tool(self, name: str, args: dict[str, Any]) -> Any:
        """Execute a tool by name.

        Args:
            name: Tool name
            args: Tool arguments

        Returns:
            Tool execution result

        Raises:
            ValueError: If no callback registered for tool
        """
        if name not in self._callbacks:
            raise ValueError(f"No callback registered for tool: {name}")

        callback = self._callbacks[name]

        if asyncio.iscoroutinefunction(callback):
            return await callback(args)
        return callback(args)
