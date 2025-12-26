"""Termin.AI LLM Client Module

This module provides LLM integration for Termin.AI using Python's
rich ecosystem of LLM libraries (PydanticAI + LiteLLM).
"""

from terminai_llm.client import LLMClient, SuggestedCommand, TerminalContext
from terminai_llm.logging_bridge import setup_rust_logging
from terminai_llm.tools import ToolRegistry

__all__ = [
    "LLMClient",
    "SuggestedCommand",
    "TerminalContext",
    "ToolRegistry",
    "setup_rust_logging",
]
__version__ = "0.1.0"
