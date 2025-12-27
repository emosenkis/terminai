"""Tests for AI agent tools."""

import tempfile
from pathlib import Path

import pytest

from terminai_agent.tools.grep_files import GrepFilesArgs, grep_files
from terminai_agent.tools.read_file import ReadFileArgs, read_file


@pytest.fixture
def temp_workspace():
    """Create a temporary workspace with test files."""
    with tempfile.TemporaryDirectory() as tmpdir:
        workspace = Path(tmpdir)

        # Create test files
        (workspace / "file1.txt").write_text("Hello, world!\nThis is a test file.\n")
        (workspace / "file2.py").write_text(
            "def hello():\n    print('Hello from Python')\n    return 42\n"
        )

        # Create subdirectory with files
        subdir = workspace / "subdir"
        subdir.mkdir()
        (subdir / "file3.txt").write_text("Another test file\nWith multiple lines\n")

        yield workspace


@pytest.mark.asyncio
async def test_read_file_success(temp_workspace):
    """Test reading a file successfully."""
    args = ReadFileArgs(path="file1.txt")
    result = await read_file(args, str(temp_workspace))

    assert result.error is None
    assert "Hello, world!" in result.content
    assert "This is a test file" in result.content
    assert result.path == "file1.txt"
    assert result.total_lines == 2


@pytest.mark.asyncio
async def test_read_file_with_line_range(temp_workspace):
    """Test reading a file with line range."""
    args = ReadFileArgs(path="file2.py", start_line=1, max_lines=2)
    result = await read_file(args, str(temp_workspace))

    assert result.error is None
    assert "print('Hello from Python')" in result.content
    assert "def hello" not in result.content
    assert result.lines_shown == (2, 3)  # Lines 2-3 (1-indexed)


@pytest.mark.asyncio
async def test_read_file_not_found(temp_workspace):
    """Test reading a non-existent file."""
    args = ReadFileArgs(path="nonexistent.txt")
    result = await read_file(args, str(temp_workspace))

    assert result.error is not None
    assert "not found" in result.error.lower()


@pytest.mark.asyncio
async def test_read_file_path_traversal(temp_workspace):
    """Test that path traversal is prevented."""
    args = ReadFileArgs(path="../../../etc/passwd")
    result = await read_file(args, str(temp_workspace))

    assert result.error is not None
    assert "traversal" in result.error.lower()


@pytest.mark.asyncio
async def test_read_file_subdirectory(temp_workspace):
    """Test reading a file in a subdirectory."""
    args = ReadFileArgs(path="subdir/file3.txt")
    result = await read_file(args, str(temp_workspace))

    assert result.error is None
    assert "Another test file" in result.content


@pytest.mark.asyncio
async def test_grep_files_simple_match(temp_workspace):
    """Test simple grep search."""
    args = GrepFilesArgs(pattern="Hello")
    result = await grep_files(args, str(temp_workspace))

    assert result.error is None
    assert len(result.matches) >= 2  # Should match in file1.txt and file2.py
    assert any("file1.txt" in m.file for m in result.matches)
    assert any("file2.py" in m.file for m in result.matches)


@pytest.mark.asyncio
async def test_grep_files_case_insensitive(temp_workspace):
    """Test case-insensitive search."""
    args = GrepFilesArgs(pattern="HELLO", case_insensitive=True)
    result = await grep_files(args, str(temp_workspace))

    assert result.error is None
    assert len(result.matches) >= 2


@pytest.mark.asyncio
async def test_grep_files_with_file_pattern(temp_workspace):
    """Test grep with file pattern filter."""
    args = GrepFilesArgs(pattern="test", file_pattern="*.txt")
    result = await grep_files(args, str(temp_workspace))

    assert result.error is None
    # Should only match .txt files
    assert all(m.file.endswith(".txt") for m in result.matches)


@pytest.mark.asyncio
async def test_grep_files_regex_pattern(temp_workspace):
    """Test grep with regex pattern."""
    args = GrepFilesArgs(pattern=r"return\s+\d+")
    result = await grep_files(args, str(temp_workspace))

    assert result.error is None
    assert len(result.matches) >= 1
    assert any("file2.py" in m.file for m in result.matches)


@pytest.mark.asyncio
async def test_grep_files_invalid_regex(temp_workspace):
    """Test grep with invalid regex."""
    args = GrepFilesArgs(pattern="[invalid(")
    result = await grep_files(args, str(temp_workspace))

    assert result.error is not None
    assert "regex" in result.error.lower()


@pytest.mark.asyncio
async def test_grep_files_max_matches(temp_workspace):
    """Test grep with max matches limit."""
    # Create many files with matches
    for i in range(20):
        (temp_workspace / f"match{i}.txt").write_text(f"test line {i}\n")

    args = GrepFilesArgs(pattern="test", max_matches=5)
    result = await grep_files(args, str(temp_workspace))

    assert len(result.matches) <= 5
    assert result.truncated


@pytest.mark.asyncio
async def test_grep_files_no_matches(temp_workspace):
    """Test grep with no matches."""
    args = GrepFilesArgs(pattern="NONEXISTENT_PATTERN_12345")
    result = await grep_files(args, str(temp_workspace))

    assert result.error is None
    assert len(result.matches) == 0
