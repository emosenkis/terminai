# Termin.AI Agent

Python subprocess for Termin.AI that provides AI assistance using Pydantic AI and AG-UI protocol.

## Development

This project uses [uv](https://github.com/astral-sh/uv) for dependency management.

### Setup

```bash
# Install dependencies
uv sync

# Run tests
uv run pytest

# Run the agent (for testing)
uv run python -m terminai_agent --secret test-secret-123
```

### Project Structure

```
terminai_agent/
├── __init__.py
├── __main__.py          # Entry point
├── server.py            # FastAPI server with AG-UI endpoints
├── agent.py             # Pydantic AI agent (Phase 2)
├── config.py            # Provider configuration (Phase 2)
└── tools/               # Agent tools (Phase 3+)
    ├── __init__.py
    ├── read_file.py     # File reading tool
    └── grep_files.py    # File searching tool
```

## Architecture

The agent runs as a subprocess of the main Termin.AI Rust process:

1. Rust spawns Python with a shared secret
2. Python selects an available port and outputs `AG_UI_PORT=<port>` to stdout
3. Rust connects to the Python server using the AG-UI protocol
4. Tools are executed either in Python or Rust depending on their requirements

See `../LLM_OVER_AG_UI_DESIGN.md` for full architecture details.
