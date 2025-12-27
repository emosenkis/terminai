"""Tool for searching files with grep-like functionality."""

import logging
import re
from pathlib import Path

from pydantic import BaseModel, Field

logger = logging.getLogger(__name__)

# Maximum number of matches to return
MAX_MATCHES = 100

# Maximum number of files to search
MAX_FILES = 1000


class GrepFilesArgs(BaseModel):
    """Arguments for grep_files tool."""

    pattern: str = Field(description="Pattern to search for (regex or literal string)")
    file_pattern: str | None = Field(
        None, description="File glob pattern (e.g., '*.rs', 'src/**/*.py'). Optional."
    )
    case_insensitive: bool = Field(False, description="Whether to use case-insensitive search")
    max_matches: int | None = Field(
        None, description=f"Maximum number of matches to return (default: 50, max: {MAX_MATCHES})"
    )


class Match(BaseModel):
    """A single match in a file."""

    file: str = Field(description="Relative path to the file")
    line_number: int = Field(description="Line number (1-indexed)")
    line: str = Field(description="Matching line content")


class GrepFilesResult(BaseModel):
    """Result from grep_files tool."""

    matches: list[Match] = Field(description="List of matches found")
    pattern: str = Field(description="Pattern that was searched")
    num_files_searched: int = Field(description="Number of files that were searched")
    truncated: bool = Field(description="Whether results were truncated")
    error: str | None = Field(None, description="Error message if search failed")


def should_skip_file(path: Path) -> bool:
    """Determine if a file should be skipped during search.

    Args:
        path: Path to check

    Returns:
        True if file should be skipped, False otherwise
    """
    # Skip hidden files and directories
    if any(part.startswith(".") for part in path.parts):
        return True

    # Skip common directories
    skip_dirs = {"node_modules", "target", "__pycache__", ".git", ".venv", "venv"}
    if any(part in skip_dirs for part in path.parts):
        return True

    # Skip binary files by extension
    binary_extensions = {".pyc", ".so", ".dylib", ".dll", ".exe", ".bin", ".o"}
    if path.suffix in binary_extensions:
        return True

    # Skip large files (> 10MB)
    try:
        if path.stat().st_size > 10 * 1024 * 1024:
            return True
    except OSError:
        return True

    return False


async def grep_files(args: GrepFilesArgs, cwd: str) -> GrepFilesResult:
    """Search for a pattern in files under the current working directory.

    Args:
        args: Tool arguments
        cwd: Current working directory

    Returns:
        GrepFilesResult with matches or error
    """
    cwd_path = Path(cwd)
    matches: list[Match] = []
    files_searched = 0
    max_matches = min(args.max_matches or 50, MAX_MATCHES)

    # Compile regex pattern
    try:
        flags = re.IGNORECASE if args.case_insensitive else 0
        pattern = re.compile(args.pattern, flags)
    except re.error as e:
        return GrepFilesResult(
            matches=[],
            pattern=args.pattern,
            num_files_searched=0,
            truncated=False,
            error=f"Invalid regex pattern: {e}",
        )

    # Determine which files to search
    if args.file_pattern:
        # Use glob pattern
        try:
            file_paths = list(cwd_path.glob(args.file_pattern))
        except ValueError as e:
            return GrepFilesResult(
                matches=[],
                pattern=args.pattern,
                num_files_searched=0,
                truncated=False,
                error=f"Invalid file pattern: {e}",
            )
    else:
        # Search all files recursively
        file_paths = list(cwd_path.rglob("*"))

    # Search files
    for file_path in file_paths:
        if len(matches) >= max_matches or files_searched >= MAX_FILES:
            break

        # Skip if not a file or should be skipped
        if not file_path.is_file() or should_skip_file(file_path):
            continue

        files_searched += 1

        try:
            # Read file and search
            content = file_path.read_text()
            for line_num, line in enumerate(content.splitlines(), start=1):
                if pattern.search(line):
                    # Get relative path from cwd
                    try:
                        relative_path = file_path.relative_to(cwd_path)
                    except ValueError:
                        relative_path = file_path

                    matches.append(
                        Match(
                            file=str(relative_path),
                            line_number=line_num,
                            line=line.rstrip(),
                        )
                    )

                    if len(matches) >= max_matches:
                        break
        except (UnicodeDecodeError, PermissionError):
            # Skip files we can't read
            continue
        except Exception as e:
            logger.debug(f"Error searching file {file_path}: {e}")
            continue

    truncated = len(matches) >= max_matches or files_searched >= MAX_FILES

    return GrepFilesResult(
        matches=matches,
        pattern=args.pattern,
        num_files_searched=files_searched,
        truncated=truncated,
    )


def format_grep_result(result: GrepFilesResult) -> str:
    """Format grep result as a human-readable string.

    Args:
        result: GrepFilesResult to format

    Returns:
        Formatted string
    """
    if result.error:
        return f"Error: {result.error}"

    if not result.matches:
        return f"No matches found for pattern: {result.pattern}"

    output = [f"## Grep Results for '{result.pattern}'\n"]
    output.append(f"Found {len(result.matches)} matches:\n")

    current_file = None
    for match in result.matches:
        # Show file path when it changes
        if current_file != match.file:
            if current_file is not None:
                output.append("\n")
            output.append(f"### {match.file}\n\n")
            current_file = match.file

        output.append(f"{match.line_number}:  {match.line}\n")

    if result.truncated:
        output.append(f"\n*Showing first {len(result.matches)} matches. There may be more.*")

    return "".join(output)
