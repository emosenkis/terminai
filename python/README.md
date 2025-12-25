# Termin.AI Python LLM Module

Python-based LLM integration for Termin.AI using PydanticAI.

## Overview

This module provides a type-safe, async-first LLM client that bridges between Termin.AI's Rust codebase and Python's rich AI ecosystem. It leverages PydanticAI for structured outputs and tool calling, with support for multiple LLM providers.

## Features

- **Multi-Provider Support**: Anthropic, OpenAI, Google Vertex, Ollama, DeepSeek, Groq
- **Type-Safe**: Full Pydantic validation for all inputs and outputs
- **Async-First**: Built for async/await with streaming support
- **Tool Calling**: Extensible tool system for LLM capabilities
- **Terminal Context**: Rich context passing from terminal state

## Installation

### Development Setup

```bash
# Install with uv (recommended)
uv sync

# Or with pip
pip install -e .

# Install dev dependencies
uv add --dev pytest pytest-asyncio ruff mypy
```

### Requirements

- Python 3.12+
- Provider API keys (see Configuration below)

## Usage

### Basic Client

```python
from terminai_llm import LLMClient, TerminalContext

# Create client
client = LLMClient(
    provider="anthropic",
    model="claude-sonnet-4-5",  # Optional, uses provider default
    api_key="sk-..."  # Optional, uses environment variable
)

# Build context
context = TerminalContext(
    cwd="/home/user/project",
    history_lines=["$ ls", "file.txt", "$ pwd"],
    last_exit_code=0
)

# Send message (async streaming)
async for chunk in client.send_message_stream(
    "How do I list hidden files?",
    {"cwd": "/tmp", "history_lines": [], "last_exit_code": None},
    []  # message history
):
    print(chunk, end="")

# Get suggested commands
commands = client.take_suggested_commands()
for cmd in commands:
    print(f"Command: {cmd['command']}")
    print(f"Explanation: {cmd['explanation']}")
```

### Supported Providers

| Provider | Model Format | Default Model |
|----------|-------------|---------------|
| Anthropic | `anthropic:model` | claude-sonnet-4-5 |
| OpenAI | `openai:model` | gpt-4 |
| Google Vertex | `google-vertex:model` | gemini-2.0-flash-exp |
| Ollama | `ollama:model` | llama3 |
| DeepSeek | `deepseek:model` | deepseek-chat |
| Groq | `groq:model` | llama-3.1-70b-versatile |

### Configuration

Set environment variables for API keys:

```bash
export ANTHROPIC_API_KEY="sk-..."
export OPENAI_API_KEY="sk-..."
export GOOGLE_API_KEY="..."
export OLLAMA_BASE_URL="http://localhost:11434"  # For Ollama
```

## Development

### Running Tests

```bash
# Run all tests
uv run pytest -v

# Run with coverage
uv run pytest --cov=terminai_llm --cov-report=html

# Run specific test file
uv run pytest tests/test_client.py -v
```

### Linting and Type Checking

```bash
# Run ruff linter
uv run ruff check .

# Auto-fix issues
uv run ruff check --fix .

# Run mypy type checker
uv run mypy terminai_llm tests
```

### Code Style

- **Type Annotations**: Full PEP-484 annotations required
- **Modern Syntax**: Use `list` over `List`, `| None` over `Optional`
- **Line Length**: 100 characters (ruff configured)
- **Async**: Prefer async/await for I/O operations

## Architecture

### Components

- **`client.py`**: Main LLMClient using PydanticAI
  - Provider abstraction
  - Streaming support
  - Context formatting
  - Tool registration

- **`tools.py`**: Tool registry and execution
  - Built-in tools (suggest_command, read_file, etc.)
  - Callback system for Rust integration
  - Sync and async tool support

### Tool System

Tools are registered with the PydanticAI agent using the `@agent.tool` decorator:

```python
@agent.tool
async def suggest_command(
    ctx: RunContext[TerminalContext],
    /,
    command: str,
    explanation: str,
    raw: bool = False,
) -> str:
    """Suggest a command for the user to execute."""
    suggested = SuggestedCommand(
        command=command,
        explanation=explanation,
        raw=raw,
    )
    self._suggested_commands.append(suggested)
    return f"✓ Command suggested: `{command}`"
```

### PyO3 Bridge

The Python module is designed to be called from Rust via PyO3:

```rust
// Rust side
let bridge = PythonLLMBridge::new(Provider::Anthropic, None).await?;
let commands = bridge.take_suggested_commands()?;
```

Python objects are converted to/from Rust types using PyO3's type conversion system.

## Testing

### Test Structure

- `tests/test_client.py`: Client functionality tests
- `tests/test_tools.py`: Tool registry tests
- `tests/test_integration.py`: Integration tests

### Test Coverage

```bash
# Current: 28/28 tests passing
# Coverage: Client, Tools, Integration
```

### Mock API Keys

Tests use mock API keys set via pytest fixtures. No actual API calls are made in unit tests.

## Troubleshooting

### Import Errors

Ensure the module is installed in development mode:
```bash
uv add -e .
```

### API Key Issues

Verify environment variables are set:
```bash
echo $ANTHROPIC_API_KEY
```

### Type Checking Errors

Run mypy in strict mode to catch type issues early:
```bash
uv run mypy --strict terminai_llm
```

## Contributing

1. Follow the code style guide (PEP-8 with ruff config)
2. Add type annotations for all public functions
3. Write tests for new functionality
4. Run linters before committing
5. Update this README for API changes

## License

MIT - See LICENSE file in repository root

## References

- [PydanticAI Documentation](https://ai.pydantic.dev/)
- [PyO3 User Guide](https://pyo3.rs/)
- [Termin.AI Design Document](../PYTHON_LLM_DESIGN.md)
