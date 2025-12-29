"""Pydantic AI agent for terminal assistance."""

import logging
from collections.abc import AsyncIterator
from typing import Any

from pydantic import BaseModel
from pydantic_ai import Agent, RunContext
from pydantic_ai.models import KnownModelName

from terminai_agent.config import Provider, ProviderConfig
from terminai_agent.tools.grep_files import GrepFilesArgs, format_grep_result, grep_files
from terminai_agent.tools.read_file import ReadFileArgs, read_file

logger = logging.getLogger(__name__)


class TerminalContext(BaseModel):
    """Context from the terminal for the AI agent."""

    history_lines: list[str]
    cwd: str
    last_exit_code: int | None = None


class Message(BaseModel):
    """A message in the conversation."""

    role: str  # "user" or "assistant"
    content: str


SYSTEM_PROMPT = """You are an AI assistant integrated into a terminal multiplexer called Termin.AI.

Your role is to help users with their terminal tasks by:
- Analyzing terminal output and providing insights
- Suggesting commands to solve problems
- Explaining errors and how to fix them
- Helping debug issues
- Automating repetitive tasks

You can use markdown formatting in your responses for better readability.

When suggesting shell commands for the user to execute:
1. Use the `suggest_command` tool (NOT markdown code blocks)
2. Provide clear explanations of what each command does and why
3. Warn about potentially dangerous operations

You have access to:
- **Terminal context:** Recent history, current directory, exit codes
- **Tools:**
  - `suggest_command`: Suggest a shell command for the user to execute
  - `read_scrollback`: Read recent terminal output
  - `read_file_tool`: Read file contents from disk (use this to examine code/config files)
  - `grep_files_tool`: Search files for patterns (use this to find code references)

Be concise but thorough. Prioritize practical solutions."""


class TerminAIAgent:
    """AI agent for terminal assistance using Pydantic AI."""

    def __init__(self, provider_config: ProviderConfig):
        """Initialize the agent.

        Args:
            provider_config: Configuration for the LLM provider
        """
        self.provider_config = provider_config
        self.agent = self._create_agent()

    def _create_agent(self) -> Agent[TerminalContext, Any]:
        """Create the Pydantic AI agent with registered tools."""
        # Map our provider to Pydantic AI model name
        model_name = self._get_model_name()

        logger.info(f"Creating agent with model: {model_name}")

        # Create agent with system prompt
        agent = Agent(
            model_name,
            system_prompt=SYSTEM_PROMPT,
        )

        # Register Python-side tools
        @agent.tool
        async def read_file_tool(
            ctx: RunContext[TerminalContext],
            path: str,
            start_line: int | None = None,
            max_lines: int | None = None,
        ) -> str:
            """Read file contents with optional line range.

            Args:
                path: Path to the file to read (relative to cwd or absolute)
                start_line: Starting line number (0-indexed, optional)
                max_lines: Maximum number of lines to read (default: 100, max: 1000)

            Returns:
                File contents as formatted string
            """
            args = ReadFileArgs(path=path, start_line=start_line, max_lines=max_lines)
            result = await read_file(args, cwd=ctx.deps.cwd)

            if result.error:
                raise ValueError(result.error)

            return result.content

        @agent.tool
        async def grep_files_tool(
            ctx: RunContext[TerminalContext],
            pattern: str,
            file_pattern: str | None = None,
            case_insensitive: bool = False,
            max_matches: int | None = None,
        ) -> str:
            """Search files for pattern using regex.

            Args:
                pattern: Pattern to search for (regex or literal string)
                file_pattern: File glob pattern (e.g., '*.rs', 'src/**/*.py'). Optional.
                case_insensitive: Whether to use case-insensitive search
                max_matches: Maximum number of matches to return (default: 50, max: 100)

            Returns:
                Formatted search results
            """
            args = GrepFilesArgs(
                pattern=pattern,
                file_pattern=file_pattern,
                case_insensitive=case_insensitive,
                max_matches=max_matches,
            )
            result = await grep_files(args, cwd=ctx.deps.cwd)

            if result.error:
                raise ValueError(result.error)

            return format_grep_result(result)

        return agent

    def _get_model_name(self) -> str | KnownModelName:
        """Get the Pydantic AI model name for our provider.

        Returns:
            Model name compatible with Pydantic AI
        """
        provider = self.provider_config.provider
        model = self.provider_config.model

        # Pydantic AI requires provider prefixes
        if provider == Provider.ANTHROPIC:
            # Ensure model has 'claude-' prefix
            if not model.startswith("claude-"):
                model = f"claude-{model}"
            return f"anthropic:{model}"
        elif provider == Provider.OPENAI:
            if not model.startswith("gpt-") and not model.startswith("o1-"):
                model = f"gpt-{model}"
            return f"openai:{model}"
        elif provider == Provider.GEMINI:
            return f"gemini:{model}"
        elif provider == Provider.OLLAMA:
            return f"ollama:{model}"
        elif provider == Provider.OPENROUTER:
            return f"openai:{model}"  # OpenRouter uses OpenAI-compatible API
        else:
            raise ValueError(f"Unsupported provider: {provider}")

    async def chat(
        self,
        user_message: str,
        context: TerminalContext,
        conversation_history: list[Message],
    ) -> str:
        """Send a chat message and get a response.

        Args:
            user_message: The user's message
            context: Terminal context
            conversation_history: Previous conversation messages

        Returns:
            The assistant's response
        """
        # Build the full prompt with context
        full_message = self._build_message_with_context(user_message, context)

        # Run the agent
        result = await self.agent.run(full_message, deps=context)

        return result.data

    async def chat_stream(
        self,
        user_message: str,
        context: TerminalContext,
        conversation_history: list[Message],
    ) -> AsyncIterator[str]:
        """Send a chat message and stream the response.

        Args:
            user_message: The user's message
            context: Terminal context
            conversation_history: Previous conversation messages

        Yields:
            Response text chunks
        """
        # Build the full prompt with context
        full_message = self._build_message_with_context(user_message, context)

        # Stream the response
        async with self.agent.run_stream(full_message, deps=context) as stream:
            async for text in stream.stream_text():
                yield text

    def _build_message_with_context(self, user_message: str, context: TerminalContext) -> str:
        """Build the full message with terminal context.

        Args:
            user_message: The user's message
            context: Terminal context

        Returns:
            Full message with context
        """
        context_parts = ["## Current Context\n"]

        # Working directory
        context_parts.append(f"**Working Directory:** `{context.cwd}`\n")

        # Last exit code
        if context.last_exit_code is not None:
            context_parts.append(f"**Last Exit Code:** {context.last_exit_code}")
            if context.last_exit_code != 0:
                context_parts.append(" (Command failed)")
            context_parts.append("\n")

        # Terminal history
        if context.history_lines:
            # Include last 50 lines or all if fewer
            lines_to_show = context.history_lines[-50:]
            context_parts.append("\n## Recent Terminal Output\n\n```\n")
            context_parts.append("\n".join(lines_to_show))
            context_parts.append("\n```\n")

        context_str = "".join(context_parts)

        return f"{context_str}\n\n{user_message}"


async def create_agent(provider: Provider | str) -> TerminAIAgent:
    """Create an AI agent for the specified provider.

    Args:
        provider: The LLM provider to use (Provider enum or string)

    Returns:
        Configured TerminAIAgent instance

    Raises:
        ValueError: If provider is invalid or API key is missing
    """
    if isinstance(provider, str):
        provider = Provider(provider.lower())

    config = ProviderConfig.from_env(provider)
    return TerminAIAgent(config)
