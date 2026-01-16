"""Tool for reading file contents."""

import logging
from pathlib import Path

from pydantic import BaseModel, Field

logger = logging.getLogger(__name__)

# Maximum number of lines that can be read from a file
MAX_FILE_LINES = 1000


class ReadFileArgs(BaseModel):
    """Arguments for read_file tool."""

    path: str = Field(
        description="Path to the file to read (relative to cwd or absolute)"
    )
    start_line: int | None = Field(
        None, description="Starting line number (0-indexed, optional)"
    )
    max_lines: int | None = Field(
        None,
        description=f"Maximum number of lines to read (default: 100, max: {MAX_FILE_LINES})",
    )


class ReadFileResult(BaseModel):
    """Result from read_file tool."""

    content: str = Field(description="File content or error message")
    path: str = Field(description="Path that was read")
    lines_shown: tuple[int, int] | None = Field(
        default=None, description="Range of lines shown (start, end) if partial read"
    )
    total_lines: int | None = Field(None, description="Total lines in file")
    error: str | None = Field(None, description="Error message if read failed")


def is_safe_path(path: Path, cwd: Path) -> bool:
    """Check if a path is safe (no path traversal outside cwd).

    Args:
        path: Path to check
        cwd: Current working directory

    Returns:
        True if path is safe, False otherwise
    """
    try:
        # Resolve the path and check if it's within cwd
        resolved = path.resolve()
        cwd_resolved = cwd.resolve()
        return resolved.is_relative_to(cwd_resolved)
    except (ValueError, OSError):
        return False


async def read_file(args: ReadFileArgs, cwd: str) -> ReadFileResult:
    """Read contents of a file by path.

    Args:
        args: Tool arguments
        cwd: Current working directory

    Returns:
        ReadFileResult with file content or error
    """
    cwd_path = Path(cwd)
    file_path = Path(args.path)

    # Resolve the path
    if file_path.is_absolute():
        full_path = file_path
    else:
        full_path = cwd_path / file_path

    # Security check: prevent path traversal
    if not is_safe_path(full_path, cwd_path):
        return ReadFileResult(
            content="",
            path=args.path,
            error=f"Path traversal detected: {args.path}",
        )

    # Check if file exists
    if not full_path.exists():
        return ReadFileResult(
            content="",
            path=args.path,
            error=f"File not found: {args.path}",
        )

    # Check if it's a file (not directory)
    if not full_path.is_file():
        return ReadFileResult(
            content="",
            path=args.path,
            error=f"Not a file: {args.path}",
        )

    try:
        # Read the file
        content = full_path.read_text()
        lines = content.splitlines()

        # Apply line range
        start_line = args.start_line or 0
        max_lines = min(args.max_lines or 100, MAX_FILE_LINES)

        if start_line >= len(lines):
            return ReadFileResult(
                content="",
                path=args.path,
                error=f"Invalid start_line: {start_line} (file has {len(lines)} lines)",
            )

        end_line = min(start_line + max_lines, len(lines))
        selected_lines = lines[start_line:end_line]

        result_content = "\n".join(selected_lines)
        total_lines = len(lines)

        # Format output
        if start_line > 0 or end_line < total_lines:
            # Partial read
            output = f"## File: {args.path}\n\n"
            output += f"Showing lines {start_line + 1}-{end_line} of {total_lines} total lines:\n\n"
            output += f"```\n{result_content}\n```"
        else:
            # Full file
            output = f"## File: {args.path}\n\n```\n{result_content}\n```"

        return ReadFileResult(
            content=output,
            path=args.path,
            lines_shown=(start_line + 1, end_line),
            total_lines=total_lines,
        )

    except UnicodeDecodeError:
        return ReadFileResult(
            content="",
            path=args.path,
            error=f"Cannot read binary file: {args.path}",
        )
    except PermissionError:
        return ReadFileResult(
            content="",
            path=args.path,
            error=f"Permission denied: {args.path}",
        )
    except Exception as e:
        logger.exception(f"Error reading file {args.path}")
        return ReadFileResult(
            content="",
            path=args.path,
            error=f"Error reading file: {e}",
        )
