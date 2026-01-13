"""Forwarded properties for AG-UI protocol."""

from pydantic import BaseModel


class TerminAIForwardedProps(BaseModel):
    """Forwarded properties sent with each AG-UI request.

    Contains runtime configuration that can change per-request.
    These are passed via the AG-UI protocol's forwardedProps field.
    """

    provider: str  # Provider name (e.g., "ollama", "anthropic", "openai")
    model: str  # Model name (e.g., "functiongemma", "claude-sonnet-4-5")
    # Terminal context fields (optional, sent when context is available)
    cwd: str | None = None  # Current working directory
    history_lines: list[str] | None = None  # Recent terminal history lines
    last_exit_code: int | None = None  # Last command exit code
    os_info: str | None = None  # Operating system information
    shell: str | None = None  # User's shell
