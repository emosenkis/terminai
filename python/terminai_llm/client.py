"""LLM client implementation using PydanticAI."""

import os
from collections.abc import AsyncIterator, Callable
from typing import Any

from pydantic import BaseModel, Field
from pydantic_ai import Agent, RunContext


class SuggestedCommand(BaseModel):
    """A command suggested by the AI assistant."""

    command: str = Field(description="The shell command to execute")
    explanation: str = Field(description="What the command does and why")
    raw: bool = Field(default=False, description="Contains raw escape sequences")


class TerminalContext(BaseModel):
    """Terminal state and recent history."""

    cwd: str
    history_lines: list[str] = Field(default_factory=list)
    last_exit_code: int | None = None


class LLMClient:
    """Main LLM client using PydanticAI with LiteLLM backend."""

    def __init__(
        self,
        provider: str,
        model: str | None = None,
        api_key: str | None = None,
    ) -> None:
        """Initialize the LLM client.

        Args:
            provider: LLM provider name (anthropic, openai, gemini, ollama, etc.)
            model: Model name (optional, uses provider default if not specified)
            api_key: API key for the provider (optional, uses environment if not specified)
        """
        self.provider = provider
        self.model_name = model or self._default_model(provider)
        self._suggested_commands: list[SuggestedCommand] = []
        self._tool_callbacks: dict[str, Callable[..., str]] = {}

        if api_key:
            self._set_api_key(api_key)

        # Set default Ollama base URL if not already configured
        if provider == "ollama" and "OLLAMA_BASE_URL" not in os.environ:
            os.environ["OLLAMA_BASE_URL"] = "http://localhost:11434/v1"

        model_id = f"{provider}:{self.model_name}"

        self.agent: Agent[TerminalContext, None] = Agent(
            model=model_id,
            system_prompt=self._get_system_prompt(),
            retries=2,
        )

        self._register_tools()

    def _default_model(self, provider: str) -> str:
        """Get default model for provider."""
        defaults = {
            "anthropic": "claude-sonnet-4-5",
            "openai": "gpt-4",
            "google-vertex": "gemini-2.0-flash-exp",
            "google-gla": "gemini-2.0-flash-exp",
            "ollama": "llama3",
            "deepseek": "deepseek-chat",
            "groq": "llama-3.1-70b-versatile",
        }
        return defaults.get(provider, "gpt-4")

    def _set_api_key(self, api_key: str) -> None:
        """Set API key for the provider."""
        key_mapping = {
            "anthropic": "ANTHROPIC_API_KEY",
            "openai": "OPENAI_API_KEY",
            "gemini": "GOOGLE_API_KEY",
            "deepseek": "DEEPSEEK_API_KEY",
            "groq": "GROQ_API_KEY",
        }
        if self.provider in key_mapping:
            os.environ[key_mapping[self.provider]] = api_key

    def _get_system_prompt(self) -> str:
        """Get the system prompt for the AI assistant."""
        return """You are an AI assistant integrated into a terminal emulator.
Your role is to help users understand their terminal output, suggest useful commands,
and provide guidance on shell operations.

You have access to the current terminal state including:
- Current working directory
- Recent terminal output
- Last command exit code

When suggesting commands:
- Use the suggest_command tool to properly format your suggestions
- Explain what each command does clearly
- Consider the user's current context
- Prefer safe, non-destructive commands when possible
"""

    def _register_tools(self) -> None:
        """Register tools with the PydanticAI agent."""

        @self.agent.tool
        async def suggest_command(
            ctx: RunContext[TerminalContext],
            /,
            command: str,
            explanation: str,
            raw: bool = False,
        ) -> str:
            """Suggest a command for the user to execute in the terminal.

            Args:
                command: The shell command to suggest (e.g., 'ls -la', 'git status')
                explanation: Clear explanation of what this command does
                raw: Set to true if command contains raw escape sequences
            """
            suggested = SuggestedCommand(
                command=command,
                explanation=explanation,
                raw=raw,
            )
            self._suggested_commands.append(suggested)

            raw_indicator = " (raw escape sequences)" if raw else ""
            return f"✓ Command suggested{raw_indicator}: `{command}`"

        @self.agent.tool
        async def read_file(
            ctx: RunContext[TerminalContext],
            /,
            path: str,
        ) -> str:
            """Read contents of a file.

            Args:
                path: Path to the file to read (relative to current directory)
            """
            if "read_file" in self._tool_callbacks:
                callback = self._tool_callbacks["read_file"]
                return callback(path)
            return "Error: read_file not yet implemented"

        @self.agent.tool
        async def read_scrollback(
            ctx: RunContext[TerminalContext],
            /,
            num_lines: int = 100,
        ) -> str:
            """Read recent lines from terminal scrollback buffer.

            Args:
                num_lines: Number of recent lines to read (default 100)
            """
            if ctx.deps.history_lines:
                lines = ctx.deps.history_lines[-num_lines:]
                return "\n".join(lines)
            return "No scrollback available"

        @self.agent.tool
        async def grep_files(
            ctx: RunContext[TerminalContext],
            /,
            pattern: str,
            file_glob: str = "*",
        ) -> str:
            """Search for pattern in files matching glob.

            Args:
                pattern: Regular expression pattern to search for
                file_glob: File glob pattern (e.g., '*.txt', 'src/**/*.rs')
            """
            if "grep_files" in self._tool_callbacks:
                callback = self._tool_callbacks["grep_files"]
                return callback(pattern, file_glob)
            return "Error: grep_files not yet implemented"

    async def send_message_stream(
        self,
        user_message: str,
        context: dict[str, Any],
        history: list[dict[str, str]],
    ) -> AsyncIterator[str]:
        """Send a message and stream the response.

        Args:
            user_message: The user's message/question
            context: Terminal context dictionary (cwd, history_lines, last_exit_code)
            history: Conversation history as list of role/content dicts

        Yields:
            Text chunks as they are generated
        """
        term_ctx = TerminalContext(
            cwd=context.get("cwd", "."),
            history_lines=context.get("history_lines", []),
            last_exit_code=context.get("last_exit_code"),
        )

        context_str = self._format_context(term_ctx)
        full_message = f"{context_str}\n\nUser: {user_message}"

        async with self.agent.run_stream(
            user_prompt=full_message,
            deps=term_ctx,
            message_history=None,
        ) as stream:
            async for chunk in stream.stream_text():
                yield chunk

    def _format_context(self, context: TerminalContext) -> str:
        """Format terminal context for the prompt."""
        parts = []

        parts.append(f"📂 Current directory: {context.cwd}")

        if context.last_exit_code is not None:
            status = "✓" if context.last_exit_code == 0 else "✗"
            parts.append(f"{status} Last exit code: {context.last_exit_code}")

        if context.history_lines:
            history = "\n".join(context.history_lines[-50:])
            parts.append(f"\n📜 Recent terminal output:\n```\n{history}\n```")

        return "\n".join(parts)

    def _convert_history(
        self,
        history: list[dict[str, str]],
    ) -> list[dict[str, str]]:
        """Convert message history to PydanticAI format."""
        return [{"role": msg["role"], "content": msg["content"]} for msg in history]

    def take_suggested_commands(self) -> list[dict[str, Any]]:
        """Get and clear suggested commands.

        Returns:
            List of suggested command dicts with command, explanation, and raw fields
        """
        commands = [
            {
                "command": cmd.command,
                "explanation": cmd.explanation,
                "raw": cmd.raw,
            }
            for cmd in self._suggested_commands
        ]
        self._suggested_commands.clear()
        return commands

    def register_tool_callback(self, tool_name: str, callback: Callable[..., str]) -> None:
        """Register a callback function for a tool.

        This allows external code (e.g., Rust bridge) to provide implementations
        for tools like read_file and grep_files.

        Args:
            tool_name: Name of the tool (e.g., 'read_file', 'grep_files')
            callback: Callable that implements the tool logic
        """
        self._tool_callbacks[tool_name] = callback
