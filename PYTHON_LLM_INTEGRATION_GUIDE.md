# Python LLM Integration Guide

**Status:** Phase 4 - In Progress
**Last Updated:** 2025-12-25

## Overview

This guide explains how to use the Python-based LLM implementation as an alternative to the default Rig-based implementation.

## Architecture

The implementation consists of three layers:

1. **Python Layer** (`python/terminai_llm/`)
   - PydanticAI-based LLM client
   - Multi-provider support (Anthropic, OpenAI, Google Vertex, Ollama, etc.)
   - Tool definitions with Pydantic validation

2. **Bridge Layer** (`src/llm/python_bridge.rs`)
   - PyO3-based Rust ↔ Python bridge
   - Manages Python runtime and GIL
   - Provides Rust-callable methods that invoke Python LLM client

3. **Adapter Layer** (`src/llm/adapter.rs`)
   - Unified API that works with both Rig and Python backends
   - Feature flag-based selection
   - Drop-in replacement for `LLMClient`

## Quick Start

### 1. Build with Python LLM Support

```bash
# Set Python path for PyO3
export PYO3_PYTHON=/var/home/eitan/projects/termin.ai/python/.venv/bin/python

# Build with python-llm feature
cargo build --features python-llm
```

### 2. Set API Keys

```bash
export ANTHROPIC_API_KEY="sk-..."
export OPENAI_API_KEY="sk-..."
export GOOGLE_API_KEY="..."
```

### 3. Use the Adapter in Code

```rust
use termin::llm::{LLMClientAdapter, Provider, TerminalContext, ChatMessage};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    // Create adapter - uses Python when python-llm feature is enabled
    let client = LLMClientAdapter::new(
        Provider::Anthropic,
        None  // Use default model
    ).await?;

    // Set context
    let context = TerminalContext::new(
        vec!["$ ls".to_string()],
        PathBuf::from("/tmp"),
        Some(0),
    );

    // Send message
    let response = client.send_message(
        "How do I list hidden files?",
        &context,
        &[]  // Empty history
    ).await?;

    println!("Response: {}", response);
    Ok(())
}
```

## Feature Flags

### Default (Rig Backend)

```bash
cargo build
```

Uses the Rig library with direct API calls to LLM providers.

### Python Backend

```bash
cargo build --features python-llm
```

Uses PydanticAI through PyO3 bridge.

## Testing

### Python Tests

```bash
cd python
uv run pytest -v
```

Current: **30/30 tests passing**

### Rust Tests

```bash
# Without python-llm
cargo test

# With python-llm
PYO3_PYTHON=python/.venv/bin/python cargo test --features python-llm
```

### Adapter Tests

```bash
# Test with ANTHROPIC_API_KEY set
cargo test adapter_test
```

## Current Limitations

### Python Backend

1. **No Streaming Support**
   - `send_message_stream()` returns single-item stream
   - True streaming requires `pyo3-async-runtimes` integration
   - Deferred to future phase

2. **Tool Callbacks**
   - `read_file` and `grep_files` tools have Rust implementations
   - Callbacks not fully wired up (requires passing Rust closures to Python)
   - Current approach: direct method calls on bridge

3. **No Custom Endpoints**
   - `new_with_endpoint()` ignores endpoint parameter for Python backend
   - Can be added if needed

### Both Backends

- All commands require user approval before execution
- API keys required for testing
- No offline/mock mode for integration tests

## Performance Comparison

**Not yet benchmarked.** TODO for Phase 4.

Expected differences:
- **Rig**: Lower latency, direct API calls
- **Python**: Slight overhead from PyO3 bridge, but more flexible

## Migration Path

### Phase 1: Coexistence (Current)

Both `LLMClient` (Rig) and `LLMClientAdapter` are available:

```rust
// Existing code continues to work
use termin::llm::LLMClient;  // Uses Rig

// New code can opt-in to adapter
use termin::llm::LLMClientAdapter;  // Uses Rig or Python based on feature
```

### Phase 2: Gradual Migration

Replace `LLMClient` with `LLMClientAdapter` in app code:

```rust
// Before
let client = LLMClient::new(provider, model).await?;

// After
let client = LLMClientAdapter::new(provider, model).await?;
```

### Phase 3: Make Python Default

Update `Cargo.toml`:

```toml
[features]
default = ["python-llm"]
python-llm = ["pyo3", "pyo3-async-runtimes"]
```

### Phase 4: Remove Rig Backend

Once Python backend is stable and performant:
1. Remove Rig dependency
2. Simplify adapter to always use Python
3. Clean up conditional compilation

## Development Workflow

### Adding a New Provider

1. **Python side** (`python/terminai_llm/client.py`):
   ```python
   def _default_model(self, provider: str) -> str:
       defaults = {
           # ...
           "new-provider": "default-model-name",
       }
   ```

2. **Rust side** (`src/llm/providers.rs`):
   ```rust
   pub enum Provider {
       // ...
       NewProvider,
   }
   ```

3. **Bridge** (`src/llm/python_bridge.rs`):
   ```rust
   impl ProviderExt for Provider {
       fn to_python_string(&self) -> &str {
           match self {
               // ...
               Provider::NewProvider => "new-provider",
           }
       }
   }
   ```

### Adding a New Tool

1. **Python side** (`python/terminai_llm/client.py`):
   ```python
   @self.agent.tool
   async def new_tool(
       ctx: RunContext[TerminalContext],
       /,
       arg: str,
   ) -> str:
       """Tool description."""
       # Implementation
   ```

2. **Rust side** (if needs Rust callback):
   - Implement in `src/llm/tools/new_tool.rs`
   - Export from `src/llm/tools/mod.rs`
   - Add `new_tool_impl()` to `PythonLLMBridge`

## Troubleshooting

### Import Errors

**Problem:** `ModuleNotFoundError: No module named 'terminai_llm'`

**Solution:**
```bash
cd python
uv sync
```

### PyO3 Version Mismatch

**Problem:** `PyO3 0.23 doesn't support Python 3.14`

**Solution:**
```bash
# Use Python 3.12 from venv
export PYO3_PYTHON=/path/to/python/.venv/bin/python
```

### Runtime Python Errors

**Problem:** `libpython3.12.so.1.0: cannot open shared object`

**Solution:**
```bash
export LD_LIBRARY_PATH=/path/to/python/.venv/lib
```

### API Key Errors

**Problem:** `Invalid API key`

**Solution:**
```bash
# Check environment variable
echo $ANTHROPIC_API_KEY

# Or pass explicitly
let client = LLMClientAdapter::new_with_api_key(
    Provider::Anthropic,
    None,
    Some("sk-...".to_string())
).await?;
```

## Next Steps

See `PYTHON_LLM_IMPLEMENTATION_PROGRESS.md` for detailed roadmap.

**Immediate priorities:**

1. Full app integration (replace `LLMClient` with `LLMClientAdapter`)
2. Performance benchmarking
3. Streaming support with `pyo3-async-runtimes`
4. Multi-provider testing

## Resources

- **Python Module:** `python/README.md`
- **Design Document:** `PYTHON_LLM_DESIGN.md`
- **Progress Tracker:** `PYTHON_LLM_IMPLEMENTATION_PROGRESS.md`
- **PydanticAI Docs:** https://ai.pydantic.dev/
- **PyO3 Guide:** https://pyo3.rs/
