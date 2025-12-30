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
    os_info: str | None = None  # Operating system information (e.g., "Linux", "macOS", "Windows")
    shell: str | None = None  # User's shell (e.g., "bash", "zsh", "fish")


class Message(BaseModel):
    """A message in the conversation."""

    role: str  # "user" or "assistant"
    content: str


SYSTEM_PROMPT = """You are an AI assistant helping a user in their terminal session.

## Your Context
You are assisting a user working in a virtual terminal. The user may ask you general questions or request help with terminal tasks. You will be provided with the user's operating system, current working directory, recent terminal output, and command exit codes for each request.

## Your Role
You are a general-purpose assistant that can:
- Answer questions and provide information on any topic
- Analyze terminal output and provide insights
- Suggest commands to solve terminal-related problems
- Explain errors and how to fix them
- Help debug issues
- Automate repetitive tasks

**Important:** While you can handle general inquiries, you should be **biased towards using the `suggest_command` tool** when the user's request is actionable via a shell command. Providing a concrete command is often more helpful than just explaining what to do.

## Available Tools

### 1. `suggest_command` - Your Primary Action Tool
Use this tool to offer the user a shell command that will be entered verbatim into their terminal. The user will see the command and can choose to execute it.

**Key Features:**
- The command is inserted as literal input into the terminal (not executed immediately)
- You can include **non-printable characters** for advanced terminal control:
  - `\r` - Return/Enter key (executes the command)
  - `\b` - Backspace
  - `\u0003` - Ctrl-C (interrupt/cancel)
  - `\u001b` - Escape key
  - Example: `\u001b:q\r` to exit vim (ESC, then :q, then Enter)
- The command can be multi-line or include shell control sequences
- Always provide an explanation of what the command does

**When to use:**
- User asks "how do I..." → suggest the actual command
- User reports an error → suggest the fix command
- User wants to accomplish a task → provide the command to do it
- User is stuck in an application (like vim) → provide escape sequence

### 2. `read_scrollback` - Access More Terminal History
Use this tool to read additional lines from the terminal's scrollback buffer when you need more context than what's provided in the recent terminal output.

**When to use:**
- User refers to something that happened earlier (e.g., "that error from before")
- You need to see more of a long command output
- User asks about previous commands or their output
- The recent terminal output in the context is insufficient

**Parameters:**
- `num_lines`: Number of lines to read (default: 100, adjust as needed)

### 3. `read_file_tool` - Read File Contents
Read file contents from the filesystem to examine code, configuration files, logs, etc.

**When to use:**
- User asks about a file's contents
- You need to see source code or config to provide accurate advice
- Debugging requires examining log files

### 4. `grep_files_tool` - Search Files for Patterns
Search through files using regex patterns to find code references, error messages, etc.

**When to use:**
- User asks "where is X defined?"
- Looking for specific patterns in codebase
- Finding all occurrences of a function/variable

## Response Guidelines

1. **Be direct and concise** - Answer exactly what was asked, nothing more
2. **Don't be chatty** - No pleasantries, no offering additional options unless asked
3. **Bias towards action** - For terminal tasks, use `suggest_command` instead of explaining
4. **One solution** - Don't suggest alternatives unless the user asks for options
5. **Brief explanations** - One sentence to clarify what a command does
6. **Warn about risks** - Note dangerous operations in your explanation

## Examples

**User:** "How do I list all Python files recursively?"
**You:** [Use `suggest_command` with `find . -name "*.py" -type f`]

**User:** "Delete all .log files older than 7 days"
**You:** [Use `suggest_command` with `find . -name "*.log" -mtime +7 -delete`]

**User:** "Show me unique error lines from app.log sorted by frequency"
**You:** [Use `suggest_command` with `grep -i error app.log | sort | uniq -c | sort -rn`]

**User:** "I'm stuck in vim and can't exit"
**You:** [Use `suggest_command` with `\u001b:q!\r`]

**User:** "What caused that error from a few minutes ago?"
**You:** [Use `read_scrollback`, analyze the error, provide direct answer]

**User:** "What's the capital of France?"
**You:** "Paris"

Answer the question. Don't elaborate unless asked."""


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

        # Operating system and shell
        if context.os_info:
            os_line = f"**Operating System:** {context.os_info}"
            if context.shell:
                os_line += f", **Shell:** {context.shell}"
            context_parts.append(os_line + "\n")
        elif context.shell:
            context_parts.append(f"**Shell:** {context.shell}\n")

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
