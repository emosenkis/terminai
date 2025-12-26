"""Logging bridge to route Python logs to Rust.

This module provides a custom logging handler that sends Python log messages
to Rust via a callback function registered from the Rust side.
"""

import logging
from collections.abc import Callable


class RustLogHandler(logging.Handler):
    """Logging handler that forwards Python logs to Rust."""

    def __init__(self, rust_log_callback: Callable[[str, str], None]) -> None:
        """Initialize the Rust log handler.

        Args:
            rust_log_callback: Callback function (level: str, message: str) -> None
                              that forwards logs to Rust logging system
        """
        super().__init__()
        self.rust_log_callback = rust_log_callback

    def emit(self, record: logging.LogRecord) -> None:
        """Emit a log record to Rust.

        Args:
            record: The log record to emit
        """
        try:
            # Format the message
            msg = self.format(record)

            # Map Python log levels to Rust log levels
            level_map = {
                logging.DEBUG: "debug",
                logging.INFO: "info",
                logging.WARNING: "warn",
                logging.ERROR: "error",
                logging.CRITICAL: "error",
            }

            rust_level = level_map.get(record.levelno, "info")

            # Forward to Rust
            self.rust_log_callback(rust_level, msg)
        except Exception:
            # Silently ignore errors in logging to avoid recursion
            pass


_rust_handler: RustLogHandler | None = None


def setup_rust_logging(rust_log_callback: Callable[[str, str], None]) -> None:
    """Setup Python logging to forward to Rust.

    Args:
        rust_log_callback: Callback function (level: str, message: str) -> None
    """
    global _rust_handler

    # Create handler if it doesn't exist
    if _rust_handler is None:
        _rust_handler = RustLogHandler(rust_log_callback)
        formatter = logging.Formatter(
            "[%(name)s] %(message)s"
        )
        _rust_handler.setFormatter(formatter)

    # Add handler to root logger
    root_logger = logging.getLogger()

    # Remove existing handler if present
    for handler in root_logger.handlers[:]:
        if isinstance(handler, RustLogHandler):
            root_logger.removeHandler(handler)

    root_logger.addHandler(_rust_handler)
    root_logger.setLevel(logging.DEBUG)

    # Also configure specific loggers
    for logger_name in ["terminai_llm", "pydantic_ai", "httpx"]:
        logger = logging.getLogger(logger_name)
        logger.setLevel(logging.DEBUG)


def get_logger(name: str) -> logging.Logger:
    """Get a logger with the given name.

    Args:
        name: Logger name

    Returns:
        Logger instance
    """
    return logging.getLogger(name)
