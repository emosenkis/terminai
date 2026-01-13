"""Claude Agent SDK adapter for terminal assistance."""

import logging
from typing import Any

from pydantic import BaseModel

from terminai_agent.config import ProviderConfig

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
  - `\\r` - Return/Enter key (executes the command)
  - `\\b` - Backspace
  - `\\u0003` - Ctrl-C (interrupt/cancel)
  - `\\u001b` - Escape key
  - Example: `\\u001b:q\\r` to exit vim (ESC, then :q, then Enter)
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

### 3. `Read` - Read File Contents
Read file contents from the filesystem to examine code, configuration files, logs, etc.

**When to use:**
- User asks about a file's contents
- You need to see source code or config to provide accurate advice
- Debugging requires examining log files

### 4. `Grep` - Search Files for Patterns
Search through files using patterns to find code references, error messages, etc.

**When to use:**
- User asks "where is X defined?"
- Looking for specific patterns in codebase
- Finding all occurrences of a function/variable

### 5. `Bash` - Execute Shell Commands
Execute shell commands and see their output. Use this to gather information or perform file operations.

**When to use:**
- Need to check file structure (ls, find)
- Need to examine system state
- Perform file operations that help answer the user's question

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
**You:** [Use `suggest_command` with `\\u001b:q!\\r`]

**User:** "What caused that error from a few minutes ago?"
**You:** [Use `read_scrollback`, analyze the error, provide direct answer]

**User:** "What's the capital of France?"
**You:** "Paris"

Answer the question. Don't elaborate unless asked."""


def create_agent_adapter(provider_config: ProviderConfig, terminal_context: TerminalContext | None = None) -> Any:
    """Create a Claude Agent SDK adapter for terminal assistance.

    Args:
        provider_config: Configuration for the LLM provider
        terminal_context: Optional terminal context

    Returns:
        Configured ClaudeAgentAdapter instance
    """
    import os
    from ag_ui_claude_sdk import ClaudeAgentAdapter

    # Build system prompt with terminal context
    system_prompt_parts = [SYSTEM_PROMPT]

    if terminal_context:
        context_parts = ["\n\n## Current Context\n"]

        # Operating system and shell
        if terminal_context.os_info:
            os_line = f"**Operating System:** {terminal_context.os_info}"
            if terminal_context.shell:
                os_line += f", **Shell:** {terminal_context.shell}"
            context_parts.append(os_line + "\n")
        elif terminal_context.shell:
            context_parts.append(f"**Shell:** {terminal_context.shell}\n")

        # Working directory
        context_parts.append(f"**Working Directory:** `{terminal_context.cwd}`\n")

        # Last exit code
        if terminal_context.last_exit_code is not None:
            context_parts.append(f"**Last Exit Code:** {terminal_context.last_exit_code}")
            if terminal_context.last_exit_code != 0:
                context_parts.append(" (Command failed)")
            context_parts.append("\n")

        # Terminal history
        if terminal_context.history_lines:
            # Include last 50 lines or all if fewer
            lines_to_show = terminal_context.history_lines[-50:]
            context_parts.append("\n## Recent Terminal Output\n\n```\n")
            context_parts.append("\n".join(lines_to_show))
            context_parts.append("\n```\n")

        system_prompt_parts.append("".join(context_parts))

    full_system_prompt = "".join(system_prompt_parts)

    # Map provider to model name
    model = provider_config.model

    # Extract API key from environment
    # NOTE: Claude SDK only supports Anthropic. For non-Anthropic providers,
    # we fall back to ANTHROPIC_API_KEY (user must configure this).
    api_key = None
    if provider_config.api_key_env:
        api_key = os.getenv(provider_config.api_key_env)
        if not api_key and provider_config.provider.value == "anthropic":
            logger.warning(f"API key environment variable {provider_config.api_key_env} is not set")

    # Warn if non-Anthropic provider requested
    if provider_config.provider.value != "anthropic":
        logger.warning(
            f"Claude SDK only supports Anthropic models. "
            f"Requested provider: {provider_config.provider.value}, model: {model}. "
            f"Falling back to ANTHROPIC_API_KEY environment variable."
        )
        # Fall back to ANTHROPIC_API_KEY for non-Anthropic providers
        api_key = os.getenv("ANTHROPIC_API_KEY")
        if not api_key:
            logger.error("ANTHROPIC_API_KEY not found. Claude SDK requires Anthropic API key.")

    # Create the adapter with system prompt and allowed tools
    adapter = ClaudeAgentAdapter(
        api_key=api_key,  # Explicitly pass API key
        model=model,
        system_prompt=full_system_prompt,
        cwd=terminal_context.cwd if terminal_context else None,
        # Allow the built-in Claude SDK tools
        allowed_tools=["Read", "Grep", "Bash"],
        # Accept edits by default for smoother UX
        permission_mode="acceptEdits",
        # Include partial messages for streaming
        include_partial_messages=True,
    )

    logger.info(f"Created Claude Agent SDK adapter with model: {model}")

    return adapter


async def create_agent(provider_config: ProviderConfig) -> Any:
    """Create an AI agent adapter for the specified provider.

    Args:
        provider_config: Configuration for the LLM provider

    Returns:
        Configured agent adapter instance

    Raises:
        ValueError: If provider is invalid or API key is missing
    """
    # For now, we create the adapter without terminal context
    # The context will be provided when running via the AG-UI protocol
    return create_agent_adapter(provider_config, terminal_context=None)
