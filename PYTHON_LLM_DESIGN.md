# Python LLM Integration Design

**Date:** 2025-12-24
**Status:** Design Proposal
**Purpose:** Replace current Rust `rig` library LLM integration with Python-based solution using PyO3

---

## Table of Contents

1. [Overview](#overview)
2. [Motivation](#motivation)
3. [Architecture](#architecture)
4. [Technology Stack](#technology-stack)
5. [Python Distribution & Packaging](#python-distribution--packaging)
6. [Component Design](#component-design)
7. [Async Communication Bridge](#async-communication-bridge)
8. [Tool Calling Mechanism](#tool-calling-mechanism)
9. [API Design](#api-design)
10. [Error Handling](#error-handling)
11. [Migration Path](#migration-path)
12. [Trade-offs and Considerations](#trade-offs-and-considerations)
13. [Implementation Phases](#implementation-phases)
14. [Testing Strategy](#testing-strategy)
15. [References](#references)

---

## Overview

This document proposes replacing the current Rust-based LLM integration (using the `rig` library) with a Python-based solution that leverages Python's mature LLM ecosystem while maintaining tight integration with Termin.AI's Rust codebase through PyO3.

### Key Goals

- **Leverage Python's LLM Ecosystem**: Access to mature libraries like PydanticAI, LiteLLM, and LangChain
- **Type Safety & Structured Outputs**: Use Pydantic for validated, type-safe LLM interactions
- **Maintain Performance**: Async communication between Rust and Python
- **Preserve Features**: Keep all existing functionality (streaming, tools, multi-provider support)
- **Improve Maintainability**: Simpler LLM integration code with better library support
- **Enable Future Extension**: Easier to add new providers, tools, and advanced agent features
- **Self-Contained Distribution**: Bundle Python without requiring system Python installation

---

## Motivation

### Why Move to Python?

**Current Challenges with Rust `rig` Library:**
1. Limited provider support compared to Python ecosystem
2. Less mature streaming and tool-calling implementations
3. Harder to keep up with rapid LLM API changes
4. Smaller community and fewer examples
5. More verbose code for LLM interactions

**Benefits of Python LLM Libraries:**
1. **LiteLLM**: Unified interface to 100+ LLM providers with automatic fallback/retry
2. **Active Development**: Python LLM libraries update within days of new provider features
3. **Rich Ecosystem**: Libraries for prompt engineering, observability, caching, etc.
4. **Better Documentation**: Extensive examples and community support
5. **Rapid Prototyping**: Easier to experiment with new features and providers

**PyO3 Makes This Feasible:**
- Near-zero overhead for function calls
- Async/await bridging between Rust and Python
- Safe memory management across language boundary
- Active maintenance and strong community support

---

## Architecture

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Termin.AI (Rust)                        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │     App      │  │   UI Layer   │  │  Terminal    │     │
│  └──────┬───────┘  └──────────────┘  └──────────────┘     │
│         │                                                    │
│  ┌──────▼─────────────────────────┐                        │
│  │  Rust LLM Bridge (PyO3)        │                        │
│  │  - LLMClientBridge             │                        │
│  │  - Async runtime bridge        │                        │
│  │  - Tool callback dispatcher    │                        │
│  └──────┬─────────────────────────┘                        │
└─────────┼─────────────────────────────────────────────────┘
          │ PyO3 FFI Boundary
          │ (pyo3-async-runtimes)
┌─────────▼─────────────────────────────────────────────────┐
│               Python LLM Module                             │
│  ┌─────────────────────────────────────────────────────┐  │
│  │  LLM Client (litellm / langchain)                   │  │
│  │  - Provider abstraction                             │  │
│  │  - Async streaming                                  │  │
│  │  - Message history management                       │  │
│  └─────────────────────────────────────────────────────┘  │
│  ┌─────────────────────────────────────────────────────┐  │
│  │  Tool Registry                                      │  │
│  │  - Tool definitions                                 │  │
│  │  - Callbacks to Rust (via PyO3)                     │  │
│  └─────────────────────────────────────────────────────┘  │
│  ┌─────────────────────────────────────────────────────┐  │
│  │  Context Manager                                    │  │
│  │  - Terminal context formatting                      │  │
│  │  - Conversation history                             │  │
│  └─────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

**Rust Side:**
- `LLMClientBridge`: PyO3 wrapper that exposes Python LLM client to Rust
- `AsyncBridge`: Converts between Tokio futures and Python asyncio
- `ToolCallbackDispatcher`: Routes tool calls from Python back to Rust implementations
- `StreamReceiver`: Receives streaming chunks from Python and forwards to UI

**Python Side:**
- `LLMClient`: Main client using LiteLLM or LangChain
- `ToolRegistry`: Registers tools and handles tool calling
- `ContextFormatter`: Formats terminal context for LLM prompts
- `ConversationManager`: Manages message history

---

## Technology Stack

### Python Libraries

#### Option 1: PydanticAI (Recommended)

**Overview:**
PydanticAI is a GenAI agent framework built by the Pydantic team, designed to bring the "FastAPI feeling" to AI agent development. It provides type-safe, structured outputs with excellent async support.

**Pros:**
- **Type Safety**: Full Pydantic validation for all inputs/outputs
- **Structured Outputs**: Native support for returning validated Pydantic models
- **Tool Calling**: First-class tool support with automatic schema extraction from docstrings
- **Multi-Provider**: Supports OpenAI, Anthropic, Gemini, DeepSeek, Groq, Ollama, and many more
- **Async-First**: Built for async/await with streaming support
- **Agent Framework**: Built-in support for multi-turn conversations and agent patterns
- **Durable Execution**: Can preserve agent progress across API failures and restarts
- **LiteLLM Integration**: Can use LiteLLM as a backend for even more providers
- **Better DX**: Similar to FastAPI - intuitive, well-documented, type-safe

**Cons:**
- Newer library (less battle-tested than LangChain)
- Slightly more opinionated architecture
- Additional dependency on Pydantic (though we likely use it already)

**Example:**
```python
from pydantic import BaseModel
from pydantic_ai import Agent

class SuggestedCommand(BaseModel):
    command: str
    explanation: str
    raw: bool = False

agent = Agent(
    'anthropic:claude-sonnet-4-5',
    result_type=SuggestedCommand,
    system_prompt="You are a terminal AI assistant..."
)

# Simple usage
result = await agent.run("How do I list files?")
print(result.data.command)  # Type-safe access

# Streaming
async with agent.run_stream("List files") as stream:
    async for chunk in stream.stream_text():
        print(chunk, end='')
    result = await stream.result()
```

#### Option 2: LiteLLM

**Overview:**
LiteLLM provides a unified interface to 100+ LLM providers with minimal abstraction overhead.

**Pros:**
- Unified interface to 100+ providers (OpenAI, Anthropic, Gemini, Ollama, etc.)
- Built-in retry/fallback logic and load balancing
- Native async/streaming support
- Cost tracking and rate limiting
- Minimal abstraction overhead
- 8ms P95 latency at 1k RPS
- Can be used as backend for PydanticAI

**Cons:**
- Less opinionated (need to build more ourselves)
- No built-in type safety or validation
- Tool calling interface varies by provider
- Need to handle structured outputs manually

**Use Case:**
- Best used **as a backend for PydanticAI** (via `pydantic-ai-litellm` package)
- Or standalone if we don't need agent features

**Example:**
```python
from litellm import acompletion

async def chat_stream(messages, model="gpt-4"):
    response = await acompletion(
        model=model,
        messages=messages,
        stream=True,
        tools=[...]
    )
    async for chunk in response:
        yield chunk
```

#### Option 3: LangChain

**Overview:**
Full-featured framework with extensive abstractions for agents, chains, and memory.

**Pros:**
- Most mature and battle-tested
- Extensive ecosystem (callbacks, tracing, memory, RAG, etc.)
- Strong tool/agent support
- Good documentation and community

**Cons:**
- Higher abstraction overhead
- More complex API
- Potentially slower
- Includes many features we don't need
- Less type-safe than PydanticAI

**Verdict:** Overkill for our use case

#### Recommendation

**Primary: PydanticAI + LiteLLM Backend**

Use PydanticAI as the main framework with LiteLLM as the model provider backend:

```python
from pydantic_ai import Agent
from pydantic_ai.models.litellm import LiteLLMModel

model = LiteLLMModel('anthropic/claude-sonnet-4-5')
agent = Agent(model, result_type=SuggestedCommand)
```

This gives us:
- ✅ Type safety and structured outputs from PydanticAI
- ✅ 100+ provider support from LiteLLM
- ✅ Best of both worlds
- ✅ Future-proof for advanced agent features

**Fallback:** If PydanticAI proves problematic, fall back to standalone LiteLLM.

### Rust Libraries

- **pyo3** `v0.22+`: Core Rust-Python bindings
- **pyo3-async-runtimes** `v0.22+`: Async bridge between Tokio and asyncio
- **tokio**: Rust async runtime (already in use)

---

## Python Distribution & Packaging

One of the key challenges with embedding Python in a Rust application is distribution: how do we ship Termin.AI without requiring users to have Python installed?

### Packaging Options Comparison

| Tool | Approach | Binary Size | Startup Time | Compatibility | Complexity | Status |
|------|----------|-------------|--------------|---------------|------------|--------|
| **PyOxidizer** | Embed interpreter, extract to memory | Medium | Fast (~50ms) | High | Medium | Active |
| **Nuitka** | Compile Python to C/binary | Small | Fastest | Medium (CPython quirks) | High | Active |
| **PyInstaller** | Bundle + extract to temp dir | Large | Slow (~200ms) | Very High | Low | Very Active |
| **System Python** | Require Python 3.9+ installed | Smallest | Fastest | Depends on system | Lowest | N/A |

### Option 1: PyOxidizer (Recommended)

**What it does:**
- Embeds a complete Python interpreter into your Rust binary
- Extracts Python modules **into memory** at runtime (not to disk)
- Produces highly portable, self-contained executables
- Can produce fully statically-linked binaries on Linux

**How it works:**
```bash
# Generate Python embedding artifacts
pyoxidizer generate-python-embedding-artifacts \
  --python-version 3.11 \
  --target x86_64-unknown-linux-gnu \
  artifacts/

# Add to Cargo.toml
[dependencies]
pyembed = { path = "artifacts/" }
```

**Rust integration:**
```rust
// In main.rs
use pyembed::{MainPythonInterpreter, OxidizedPythonInterpreterConfig};

fn main() -> Result<()> {
    let config = OxidizedPythonInterpreterConfig::default();
    let interp = MainPythonInterpreter::new(config)?;

    interp.with_gil(|py| {
        // Your PyO3 code here
        let terminai_llm = PyModule::import(py, "terminai_llm")?;
        // ...
    })?;

    Ok(())
}
```

**Pros:**
- ✅ Single binary distribution (or binary + small resource files)
- ✅ Fast startup - modules loaded from memory
- ✅ No temp directory extraction
- ✅ Works on systems without Python
- ✅ Supports static linking on Linux
- ✅ Good for security (no file extraction to disk)

**Cons:**
- ❌ Complex build process
- ❌ Need to build all C extension dependencies from source
- ❌ Limited support for some Python packages (those with complex native extensions)
- ❌ Larger learning curve

**Best for:** Production distribution where self-contained binary is important

**Resources:**
- [PyOxidizer Documentation](https://pyoxidizer.readthedocs.io/)
- [Generic Python Embedding in Rust](https://pyoxidizer.readthedocs.io/en/stable/pyoxidizer_rust_generic_embedding.html)

### Option 2: Nuitka

**What it does:**
- Compiles Python code to C, then compiles to native binary
- Produces fastest runtime performance
- Can create standalone executables or onefile bundles

**How it works:**
```bash
# Compile Python module to extension module
nuitka3 --module terminai_llm/

# Or create standalone
nuitka3 --standalone --onefile terminai_llm_runner.py
```

**Integration strategy:**
1. Compile Python LLM client to C extension with Nuitka
2. Link the extension into Rust binary
3. Load via PyO3

**Pros:**
- ✅ Fastest runtime performance (true compiled code)
- ✅ Can produce single-file executables
- ✅ Smaller binary size than PyOxidizer
- ✅ Compatible with Python 3.4-3.13

**Cons:**
- ❌ Requires C++ compiler at build time
- ❌ Compatibility issues with some Python code (relies on CPython internals)
- ❌ Complex build setup
- ❌ Commercial license needed for some features (data file embedding)
- ❌ Debugging compiled code is harder

**Best for:** Maximum performance, if compatibility is verified

**Resources:**
- [Nuitka GitHub](https://github.com/Nuitka/Nuitka)
- [Nuitka Use Cases](https://nuitka.net/user-documentation/use-cases.html)

### Option 3: System Python Requirement

**What it does:**
- Require users to have Python 3.9+ installed on their system
- Use `pyo3`'s auto-initialize feature to find and use system Python

**How it works:**
```toml
# Cargo.toml
[dependencies]
pyo3 = { version = "0.22", features = ["auto-initialize"] }
```

```rust
// Rust code - automatic
pyo3::prepare_freethreaded_python();
Python::with_gil(|py| {
    // Use system Python automatically
});
```

**Installation script:**
```bash
#!/bin/bash
# install.sh

# Check Python version
if ! python3 --version | grep -q "Python 3.9\|Python 3.10\|Python 3.11\|Python 3.12"; then
    echo "Error: Python 3.9+ required"
    exit 1
fi

# Install Python dependencies
pip3 install --user pydantic-ai litellm

# Install Termin.AI binary
cargo install --path .
```

**Pros:**
- ✅ Simplest build process
- ✅ Smallest binary size (~20MB)
- ✅ Fastest development iteration
- ✅ Easy to update Python dependencies
- ✅ Full compatibility with all Python packages
- ✅ Standard Python debugging tools work

**Cons:**
- ❌ Requires users to install Python
- ❌ Potential version conflicts on user's system
- ❌ More complex installation instructions
- ❌ Dependency on user's Python environment

**Best for:** Development and for users comfortable with Python

### Option 4: PyInstaller (Not Recommended)

**What it does:**
- Bundles Python interpreter and dependencies into a folder
- Extracts to temporary directory at runtime
- Can create "onefile" executable that extracts on every run

**Why not recommended:**
- Slow startup time (200ms+ for extraction)
- Antivirus false positives (common issue)
- Large distribution size
- Designed for Python apps, awkward for Rust apps calling Python

**Only consider if:** You need maximum Python compatibility and can't use PyOxidizer

**Resources:**
- [PyInstaller vs PyOxidizer Comparison](https://pyoxidizer.readthedocs.io/en/stable/pyoxidizer_comparisons.html)

### Option 5: Maturin (Reverse Direction)

**Note:** Maturin is for the opposite use case - packaging **Rust code as a Python package**, not for embedding Python in Rust binaries.

**What it does:**
- Builds Rust extensions for Python using PyO3
- Publishes to PyPI as Python wheels
- Users install via `pip install terminai`

**Not suitable for our use case** because:
- We want a standalone terminal binary, not a Python package
- Would require users to use Python to launch Termin.AI
- Goes against the goal of Rust-native application

**Resources:**
- [Maturin User Guide](https://www.maturin.rs/)

### Recommended Strategy

**Phase 1 - Development (Immediate):**
- Use **System Python** requirement
- Simple, fast iteration
- Document Python 3.9+ requirement

**Phase 2 - Alpha/Beta (Months 1-3):**
- Continue with System Python
- Gather feedback on which Python packages users need
- Test PyOxidizer with our actual dependencies

**Phase 3 - Production Release (Month 4+):**
- Switch to **PyOxidizer** for main distribution
- Provide instructions for System Python alternative
- Consider Nuitka for performance-critical deployments

**Distribution Matrix:**

| Platform | Primary Distribution | Alternative |
|----------|---------------------|-------------|
| Linux | PyOxidizer (static binary) | System Python |
| macOS | PyOxidizer (universal binary) | Homebrew (with Python dep) |
| Windows | PyOxidizer (exe) | System Python |

### Hybrid Approach: Feature Flag

Support both embedded and system Python:

```toml
# Cargo.toml
[features]
default = ["embedded-python"]
embedded-python = ["pyembed"]
system-python = []

[dependencies]
pyo3 = "0.22"
pyembed = { path = "artifacts/", optional = true }
```

```rust
// src/python_runtime.rs
#[cfg(feature = "embedded-python")]
fn initialize_python() -> Result<()> {
    use pyembed::MainPythonInterpreter;
    let interp = MainPythonInterpreter::new(Default::default())?;
    // ...
}

#[cfg(feature = "system-python")]
fn initialize_python() -> Result<()> {
    pyo3::prepare_freethreaded_python();
    // ...
}
```

Build variants:
```bash
# Embedded Python (for distribution)
cargo build --release --features embedded-python

# System Python (for development)
cargo build --release --features system-python
```

### Dependency Installation

For Python dependencies, use one of these strategies:

**Strategy 1: Vendored wheels (PyOxidizer)**
```python
# pyoxidizer.bzl
def make_exe():
    dist = default_python_distribution()
    policy = dist.make_python_packaging_policy()

    # Bundle specific packages
    policy.resources_location = "in-memory"
    exe = dist.to_python_executable(
        name="terminai",
        packaging_policy=policy,
        config=python_config,
    )

    exe.add_python_resources(dist.pip_install([
        "pydantic-ai==0.0.14",
        "litellm==1.50.3",
        "pydantic==2.9.2",
    ]))

    return exe
```

**Strategy 2: Runtime pip install (System Python)**
```python
# python/setup_check.py
import subprocess
import sys

required_packages = {
    'pydantic_ai': '0.0.14',
    'litellm': '1.50.3',
    'pydantic': '2.9.2',
}

def check_and_install():
    for package, version in required_packages.items():
        try:
            __import__(package.replace('-', '_'))
        except ImportError:
            print(f"Installing {package}...")
            subprocess.check_call([
                sys.executable, "-m", "pip", "install",
                f"{package}=={version}"
            ])
```

---

## Component Design

### 1. Rust: LLMClientBridge

**File:** `src/llm/python_bridge.rs`

```rust
use pyo3::prelude::*;
use pyo3_async_runtimes::tokio::future_into_py;
use futures::stream::{Stream, StreamExt};
use std::pin::Pin;

pub struct LLMClientBridge {
    /// Python LLM client instance
    py_client: Py<PyAny>,
    /// Python module reference
    py_module: Py<PyAny>,
}

impl LLMClientBridge {
    /// Initialize the Python LLM client
    pub async fn new(
        provider: Provider,
        model: Option<String>,
        api_key: Option<String>,
    ) -> Result<Self> {
        Python::with_gil(|py| {
            // Import Python module
            let module = PyModule::import(py, "terminai_llm")?;

            // Create client instance
            let py_client = module
                .getattr("LLMClient")?
                .call1((
                    provider.to_string(),
                    model,
                    api_key,
                ))?
                .into_py(py);

            Ok(Self {
                py_client,
                py_module: module.into_py(py),
            })
        })
    }

    /// Send a message and get streaming response
    pub async fn send_message_stream(
        &self,
        user_message: &str,
        context: &TerminalContext,
        history: &[ChatMessage],
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        // Convert context and history to Python objects
        let (py_context, py_history) = Python::with_gil(|py| {
            let ctx = self.rust_context_to_py(py, context)?;
            let hist = self.rust_messages_to_py(py, history)?;
            Ok::<_, anyhow::Error>((ctx, hist))
        })?;

        // Call Python async method and convert to Rust stream
        let stream = pyo3_async_runtimes::tokio::into_stream(
            Python::with_gil(|py| {
                self.py_client
                    .getattr(py, "send_message_stream")?
                    .call1(py, (user_message, py_context, py_history))?
                    .extract(py)
            })?
        );

        Ok(Box::pin(stream))
    }

    /// Get suggested commands from tool calls
    pub async fn take_suggested_commands(&self) -> Result<Vec<SuggestedCommand>> {
        Python::with_gil(|py| {
            let py_commands = self.py_client
                .getattr(py, "take_suggested_commands")?
                .call0(py)?;

            // Convert Python list to Rust Vec
            self.py_commands_to_rust(py, py_commands)
        })
    }

    // Helper methods for Python<->Rust conversions
    fn rust_context_to_py(&self, py: Python, ctx: &TerminalContext) -> PyResult<PyObject> {
        // Convert TerminalContext to Python dict
        let dict = pyo3::types::PyDict::new(py);
        dict.set_item("history_lines", ctx.history_lines.clone())?;
        dict.set_item("cwd", ctx.cwd.to_string_lossy().to_string())?;
        dict.set_item("last_exit_code", ctx.last_exit_code)?;
        Ok(dict.into())
    }

    fn rust_messages_to_py(&self, py: Python, msgs: &[ChatMessage]) -> PyResult<PyObject> {
        // Convert Vec<ChatMessage> to Python list of dicts
        let list = pyo3::types::PyList::empty(py);
        for msg in msgs {
            let dict = pyo3::types::PyDict::new(py);
            dict.set_item("role", &msg.role)?;
            dict.set_item("content", &msg.content)?;
            list.append(dict)?;
        }
        Ok(list.into())
    }
}
```

### 2. Python: LLM Client Module

**File:** `python/terminai_llm/__init__.py`

```python
"""Termin.AI LLM Client Module

This module provides LLM integration for Termin.AI using Python's
rich ecosystem of LLM libraries.
"""

from .client import LLMClient
from .tools import ToolRegistry
from .context import ContextFormatter

__all__ = ["LLMClient", "ToolRegistry", "ContextFormatter"]
```

**File:** `python/terminai_llm/client.py`

```python
import os
from typing import AsyncIterator, List, Dict, Optional, Any
from pydantic import BaseModel, Field
from pydantic_ai import Agent, RunContext
from pydantic_ai.models.litellm import LiteLLMModel

# Data models
class SuggestedCommand(BaseModel):
    """A command suggested by the AI assistant."""
    command: str = Field(description="The shell command to execute")
    explanation: str = Field(description="What the command does and why")
    raw: bool = Field(default=False, description="Contains raw escape sequences")

class TerminalContext(BaseModel):
    """Terminal state and recent history."""
    cwd: str
    history_lines: List[str] = Field(default_factory=list)
    last_exit_code: Optional[int] = None

class LLMClient:
    """Main LLM client using PydanticAI with LiteLLM backend."""

    def __init__(
        self,
        provider: str,
        model: Optional[str] = None,
        api_key: Optional[str] = None,
    ):
        self.provider = provider
        self.model_name = model or self._default_model(provider)
        self._suggested_commands: List[SuggestedCommand] = []

        # Set API key if provided
        if api_key:
            self._set_api_key(api_key)

        # Create LiteLLM model
        model_id = f"{provider}/{self.model_name}"
        self.model = LiteLLMModel(model_id)

        # Create PydanticAI agent
        self.agent = Agent(
            model=self.model,
            system_prompt=self._get_system_prompt(),
            retries=2,  # Built-in retry logic
        )

        # Register tools
        self._register_tools()

    def _default_model(self, provider: str) -> str:
        """Get default model for provider."""
        defaults = {
            "anthropic": "claude-sonnet-4-5",
            "openai": "gpt-4",
            "gemini": "gemini-2.5-pro",
            "ollama": "llama3",
        }
        return defaults.get(provider, "gpt-4")

    def _set_api_key(self, api_key: str):
        """Set API key for the provider."""
        key_mapping = {
            "anthropic": "ANTHROPIC_API_KEY",
            "openai": "OPENAI_API_KEY",
            "gemini": "GOOGLE_API_KEY",
        }
        if self.provider in key_mapping:
            os.environ[key_mapping[self.provider]] = api_key

    def _get_system_prompt(self) -> str:
        """Get the system prompt for the AI assistant."""
        return """You are an AI assistant integrated into a terminal emulator.
Your role is to help users understand their terminal output, suggest useful commands,
and provide guidance on shell operations.

You have access to the current terminal state including:
- Current working directory
- Recent terminal output
- Last command exit code

When suggesting commands:
- Use the suggest_command tool to properly format your suggestions
- Explain what each command does clearly
- Consider the user's current context
- Prefer safe, non-destructive commands when possible"""

    def _register_tools(self):
        """Register tools with the PydanticAI agent."""

        @self.agent.tool
        async def suggest_command(
            ctx: RunContext[TerminalContext],
            command: str,
            explanation: str,
            raw: bool = False,
        ) -> str:
            """Suggest a command for the user to execute in the terminal.

            Args:
                command: The shell command to suggest (e.g., 'ls -la', 'git status')
                explanation: Clear explanation of what this command does
                raw: Set to true if command contains raw escape sequences
            """
            suggested = SuggestedCommand(
                command=command,
                explanation=explanation,
                raw=raw,
            )
            self._suggested_commands.append(suggested)

            raw_indicator = " (raw escape sequences)" if raw else ""
            return f"✓ Command suggested{raw_indicator}: `{command}`"

        # Placeholder tools that will be implemented in Rust
        @self.agent.tool
        async def read_file(
            ctx: RunContext[TerminalContext],
            path: str,
        ) -> str:
            """Read contents of a file.

            Args:
                path: Path to the file to read (relative to current directory)
            """
            # This will be overridden with Rust callback
            return f"Error: read_file not yet implemented"

        @self.agent.tool
        async def read_scrollback(
            ctx: RunContext[TerminalContext],
            num_lines: int = 100,
        ) -> str:
            """Read recent lines from terminal scrollback buffer.

            Args:
                num_lines: Number of recent lines to read (default 100)
            """
            # This will be overridden with Rust callback
            if ctx.deps.history_lines:
                lines = ctx.deps.history_lines[-num_lines:]
                return "\n".join(lines)
            return "No scrollback available"

        @self.agent.tool
        async def grep_files(
            ctx: RunContext[TerminalContext],
            pattern: str,
            file_glob: str = "*",
        ) -> str:
            """Search for pattern in files matching glob.

            Args:
                pattern: Regular expression pattern to search for
                file_glob: File glob pattern (e.g., '*.txt', 'src/**/*.rs')
            """
            # This will be overridden with Rust callback
            return f"Error: grep_files not yet implemented"

    async def send_message_stream(
        self,
        user_message: str,
        context: Dict[str, Any],
        history: List[Dict[str, str]],
    ) -> AsyncIterator[str]:
        """Send a message and stream the response."""

        # Convert context dict to Pydantic model
        term_ctx = TerminalContext(
            cwd=context.get("cwd", "."),
            history_lines=context.get("history_lines", []),
            last_exit_code=context.get("last_exit_code"),
        )

        # Format message with context
        context_str = self._format_context(term_ctx)
        full_message = f"{context_str}\n\nUser: {user_message}"

        # Stream response using PydanticAI
        async with self.agent.run_stream(
            full_message,
            deps=term_ctx,
            message_history=self._convert_history(history),
        ) as stream:
            # Stream text chunks
            async for chunk in stream.stream_text():
                yield chunk

    def _format_context(self, context: TerminalContext) -> str:
        """Format terminal context for the prompt."""
        parts = []

        parts.append(f"📂 Current directory: {context.cwd}")

        if context.last_exit_code is not None:
            status = "✓" if context.last_exit_code == 0 else "✗"
            parts.append(f"{status} Last exit code: {context.last_exit_code}")

        if context.history_lines:
            history = "\n".join(context.history_lines[-50:])  # Last 50 lines
            parts.append(f"\n📜 Recent terminal output:\n```\n{history}\n```")

        return "\n".join(parts)

    def _convert_history(
        self,
        history: List[Dict[str, str]],
    ) -> List[Dict[str, str]]:
        """Convert message history to PydanticAI format."""
        # PydanticAI expects simple role/content dicts
        return [
            {"role": msg["role"], "content": msg["content"]}
            for msg in history
        ]

    def take_suggested_commands(self) -> List[Dict[str, Any]]:
        """Get and clear suggested commands."""
        commands = [
            {
                "command": cmd.command,
                "explanation": cmd.explanation,
                "raw": cmd.raw,
            }
            for cmd in self._suggested_commands
        ]
        self._suggested_commands.clear()
        return commands
```

**File:** `python/terminai_llm/tools.py`

```python
from typing import Dict, List, Any, Callable, Awaitable
import asyncio

class ToolRegistry:
    """Registry for LLM tools that can call back to Rust."""

    def __init__(self):
        self._tools: Dict[str, Dict[str, Any]] = {}
        self._callbacks: Dict[str, Callable] = {}

        # Register built-in tools
        self._register_builtin_tools()

    def _register_builtin_tools(self):
        """Register built-in tools."""

        # suggest_command tool
        self.register_tool(
            name="suggest_command",
            description="Suggest a command for the user to execute in the terminal",
            parameters={
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to suggest",
                    },
                    "explanation": {
                        "type": "string",
                        "description": "Explanation of what the command does",
                    },
                    "raw": {
                        "type": "boolean",
                        "description": "Whether command contains raw escape sequences",
                        "default": False,
                    },
                },
                "required": ["command", "explanation"],
            },
            callback=self._suggest_command_callback,
        )

        # read_file tool (calls back to Rust)
        self.register_tool(
            name="read_file",
            description="Read contents of a file",
            parameters={
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to read",
                    },
                },
                "required": ["path"],
            },
            callback=None,  # Will be set by Rust
        )

    def register_tool(
        self,
        name: str,
        description: str,
        parameters: Dict[str, Any],
        callback: Optional[Callable] = None,
    ):
        """Register a new tool."""
        self._tools[name] = {
            "type": "function",
            "function": {
                "name": name,
                "description": description,
                "parameters": parameters,
            },
        }
        if callback:
            self._callbacks[name] = callback

    def get_tool_definitions(self) -> List[Dict[str, Any]]:
        """Get all tool definitions for LLM API."""
        return list(self._tools.values())

    async def execute_tool(self, name: str, args: Dict[str, Any]) -> Any:
        """Execute a tool by name."""
        if name not in self._callbacks:
            raise ValueError(f"No callback registered for tool: {name}")

        callback = self._callbacks[name]

        # Handle both sync and async callbacks
        if asyncio.iscoroutinefunction(callback):
            return await callback(args)
        else:
            return callback(args)

    async def _suggest_command_callback(self, args: Dict[str, Any]) -> str:
        """Built-in callback for suggest_command tool."""
        # This is handled by storing in client._suggested_commands
        # Return confirmation message
        raw_indicator = " (contains escape sequences)" if args.get("raw") else ""
        return f"Command suggested{raw_indicator}: `{args['command']}`"
```

---

## Async Communication Bridge

### Tokio ↔ asyncio Bridge

The `pyo3-async-runtimes` library handles the async bridge:

**Key Patterns:**

1. **Python coroutine → Rust future:**
```rust
use pyo3_async_runtimes::tokio::into_future;

let rust_future = Python::with_gil(|py| {
    let py_coroutine = py_client.call_method0(py, "async_method")?;
    into_future(py_coroutine.as_ref(py))
})?;

let result = rust_future.await?;
```

2. **Rust future → Python coroutine:**
```rust
use pyo3_async_runtimes::tokio::future_into_py;

#[pyfunction]
fn rust_async_function(py: Python) -> PyResult<&PyAny> {
    future_into_py(py, async move {
        // Rust async code
        Ok(result)
    })
}
```

3. **Streaming (Python async generator → Rust Stream):**
```rust
use pyo3_async_runtimes::tokio::into_stream;

let rust_stream = Python::with_gil(|py| {
    let py_async_gen = py_client.call_method0(py, "stream_method")?;
    into_stream(py_async_gen.as_ref(py))
})?;

while let Some(chunk) = rust_stream.next().await {
    // Process chunk
}
```

### Thread Management

Following PyO3 best practices:

1. **Main thread belongs to Python** (for asyncio)
2. **Tokio runs in background threads**
3. **GIL is released during I/O operations**

**Initialization pattern:**

```rust
// In main.rs
use pyo3::prelude::*;
use pyo3_async_runtimes::tokio::init_multi_thread;

fn main() -> Result<()> {
    // Initialize Python interpreter
    pyo3::prepare_freethreaded_python();

    // Initialize asyncio event loop
    Python::with_gil(|py| {
        init_multi_thread(py)?;
        Ok(())
    })?;

    // Start Tokio runtime
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async_main())
}
```

---

## Tool Calling Mechanism

### Tool Execution Flow

```
┌─────────────┐
│   Python    │  1. LLM returns tool_call in response
│  LLM Client │────────────────────────┐
└─────────────┘                        │
                                       │
┌─────────────┐                        ▼
│   Python    │  2. ToolRegistry looks up tool callback
│ ToolRegistry│────────────────────────┐
└─────────────┘                        │
                                       │
       │                               ▼
       │ 3a. For Python tools: execute locally
       │
       ▼ 3b. For Rust tools: call back via PyO3
┌─────────────┐
│    Rust     │  4. Execute tool (e.g., read_file)
│  Tool Impl  │────────┐
└─────────────┘        │
                       │
                       ▼
                  5. Return result to Python
                       │
                       ▼
┌─────────────┐
│   Python    │  6. Send result back to LLM
│  LLM Client │
└─────────────┘
```

### Registering Rust Tools from Python

**In Rust:**

```rust
impl LLMClientBridge {
    pub fn register_tool_callback(
        &mut self,
        tool_name: &str,
        callback: Box<dyn Fn(serde_json::Value) -> BoxFuture<'static, Result<String>> + Send + Sync>,
    ) -> Result<()> {
        Python::with_gil(|py| {
            // Create Python wrapper for Rust callback
            let py_callback = PyCFunction::new_closure(
                py,
                None,
                None,
                move |args: &PyTuple, _kwargs: Option<&PyDict>| {
                    let args_json: String = args.get_item(0)?.extract()?;
                    let args_value: serde_json::Value = serde_json::from_str(&args_json)?;

                    // Call Rust callback asynchronously
                    let future = callback(args_value);
                    future_into_py(py, future)
                },
            )?;

            // Register with Python tool registry
            self.py_client
                .getattr(py, "tool_registry")?
                .call_method1(py, "register_callback", (tool_name, py_callback))?;

            Ok(())
        })
    }
}
```

**Usage:**

```rust
let mut bridge = LLMClientBridge::new(...).await?;

// Register read_file tool
bridge.register_tool_callback("read_file", Box::new(|args| {
    Box::pin(async move {
        let path = args["path"].as_str().unwrap();
        // Read file implementation
        Ok(file_contents)
    })
}))?;
```

---

## API Design

### Rust Public API

The public API remains similar to the current implementation:

```rust
// In src/llm/mod.rs

pub struct LLMClient {
    bridge: LLMClientBridge,
}

impl LLMClient {
    pub async fn new(provider: Provider, model: Option<String>) -> Result<Self>;

    pub async fn send_message(
        &self,
        user_message: &str,
        context: &TerminalContext,
        conversation_history: &[ChatMessage],
    ) -> Result<String>;

    pub async fn send_message_stream(
        &self,
        user_message: &str,
        context: &TerminalContext,
        conversation_history: &[ChatMessage],
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>>;

    pub fn take_suggested_commands(&self) -> Result<Vec<SuggestedCommand>>;

    pub fn set_cwd(&self, cwd: PathBuf) -> Result<()>;

    pub fn update_scrollback(&self, lines: Vec<String>) -> Result<()>;
}
```

**Key difference:** Implementation uses `LLMClientBridge` internally instead of direct `rig` calls.

---

## Error Handling

### Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum LLMError {
    #[error("Python error: {0}")]
    PythonError(#[from] PyErr),

    #[error("API error: {0}")]
    ApiError(String),

    #[error("Stream error: {0}")]
    StreamError(String),

    #[error("Tool execution error: {0}")]
    ToolError(String),

    #[error("Context conversion error: {0}")]
    ConversionError(String),
}
```

### Python Error Handling

```python
class LLMError(Exception):
    """Base class for LLM errors."""
    pass

class APIError(LLMError):
    """Error calling LLM API."""
    pass

class ToolExecutionError(LLMError):
    """Error executing tool."""
    pass
```

### Error Propagation

Python exceptions are automatically converted to Rust `PyErr`, which we wrap in `LLMError::PythonError`.

---

## Migration Path

### Phase 1: Parallel Implementation (Week 1)

- [ ] Set up Python module structure
- [ ] Implement PydanticAI client with LiteLLM backend
- [ ] Create PyO3 bridge with simple test
- [ ] Verify async communication works

**Deliverable:** Can call Python LLM client from Rust and get response

### Phase 2: Feature Parity (Week 2)

- [ ] Implement streaming support
- [ ] Port all existing tools to Python
- [ ] Set up tool callback mechanism
- [ ] Add multi-provider support

**Deliverable:** Python implementation has all features of Rust version

### Phase 3: Integration (Week 3)

- [ ] Create adapter layer for backward compatibility
- [ ] Add feature flag to switch between implementations
- [ ] Integration testing with full app
- [ ] Performance benchmarking

**Deliverable:** Can run app with either Rust or Python LLM backend

### Phase 4: Migration (Week 4)

- [ ] Make Python implementation the default
- [ ] Remove old Rust `rig` implementation
- [ ] Clean up API surface
- [ ] Update documentation

**Deliverable:** Fully migrated to Python LLM implementation

### Phase 5: Optimization (Week 5)

- [ ] Profile performance bottlenecks
- [ ] Optimize Python<->Rust conversions
- [ ] Add caching where beneficial
- [ ] Tune async runtime configuration

**Deliverable:** Performance meets or exceeds old implementation

---

## Trade-offs and Considerations

### Advantages ✅

1. **Better Library Support:**
   - PydanticAI + LiteLLM: 100+ providers vs ~5 in rig
   - Updates quickly when providers add features
   - Built-in retry, fallback, caching, and durable execution

2. **Type Safety:**
   - Pydantic validation for all LLM inputs and outputs
   - Structured outputs with automatic validation
   - Compile-time type checking in Python
   - Less runtime errors

3. **Easier Maintenance:**
   - Simpler, more intuitive code (FastAPI-like DX)
   - Extensive examples and documentation
   - Larger community for support
   - Better tooling integration with Rust's type system

4. **Future Flexibility:**
   - Easy to add new providers (LiteLLM supports 100+)
   - Access to Python AI ecosystem (RAG, agents, embeddings)
   - Can leverage Python-only features quickly
   - Agent framework ready for multi-agent patterns

5. **Development Speed:**
   - Faster iteration on LLM features
   - Better debugging tools (Python debugger + Pydantic error messages)
   - Rich ecosystem for observability
   - Type-safe tool definitions with automatic schema extraction

### Disadvantages ⚠️

1. **Additional Dependency:**
   - Requires Python runtime
   - Users must have Python 3.9+ installed
   - Larger distribution size

2. **Complexity:**
   - PyO3 bridge adds complexity
   - Two languages to maintain
   - More build configuration

3. **Performance Considerations:**
   - FFI overhead for each call
   - GIL contention (mitigated by async)
   - Potential memory overhead

4. **Distribution:**
   - Need to bundle Python dependencies
   - Cross-platform Python can be tricky
   - More complex CI/CD

### Mitigation Strategies

**For Python Dependency:**
- Use embedded Python interpreter (PyOxidizer)
- Or require Python as system dependency (simpler)
- Document installation clearly

**For Performance:**
- Batch Python calls where possible
- Release GIL during I/O
- Profile and optimize hot paths
- Cache Python objects across calls

**For Distribution:**
- Use PyOxidizer for single-binary distribution
- Or create install script that sets up virtualenv
- Provide platform-specific packages

---

## Implementation Phases

### Phase 0: Setup (3 days)

**Goals:**
- Python module structure
- PyO3 integration in Cargo.toml
- Basic bridge with "hello world"

**Tasks:**
1. Create `python/terminai_llm/` directory structure
2. Add `pyo3` and `pyo3-async-runtimes` to Cargo.toml
3. Create basic `LLMClientBridge` struct
4. Write simple test: Rust calls Python function
5. Set up Python development environment

**Success Criteria:**
- Can call Python code from Rust
- Tests pass

### Phase 1: Core Client (1 week)

**Goals:**
- Working LiteLLM client in Python
- Async message sending from Rust
- Basic streaming support

**Tasks:**
1. Implement `LLMClient` class with LiteLLM
2. Add context formatting
3. Create async bridge for `send_message`
4. Implement streaming with `send_message_stream`
5. Add error handling

**Success Criteria:**
- Can send message from Rust and get response
- Streaming works end-to-end
- Error cases handled gracefully

### Phase 2: Tools (1 week)

**Goals:**
- Tool calling works
- All existing tools ported
- Tool callbacks work from Python to Rust

**Tasks:**
1. Implement `ToolRegistry` in Python
2. Port `suggest_command` tool
3. Port `read_file` tool with Rust callback
4. Port `read_scrollback` tool
5. Port `grep_files` tool
6. Test all tools in integration

**Success Criteria:**
- All tools work as before
- Tool results flow correctly
- No regressions in functionality

### Phase 3: Multi-Provider (3 days)

**Goals:**
- All providers work
- Provider switching is smooth
- API key handling works

**Tasks:**
1. Test Anthropic provider
2. Test OpenAI provider
3. Test Gemini provider
4. Test Ollama provider
5. Add provider-specific configuration

**Success Criteria:**
- Can switch between providers
- Each provider works correctly
- API keys handled securely

### Phase 4: Integration (1 week)

**Goals:**
- Works in full Termin.AI app
- Feature flag for switching implementations
- Performance acceptable

**Tasks:**
1. Create adapter layer in `src/llm/mod.rs`
2. Add `python-llm` feature flag
3. Integration test with UI
4. Performance benchmarking
5. Fix any issues discovered

**Success Criteria:**
- App works with Python backend
- Performance within 10% of Rust version
- All features functional

### Phase 5: Migration (3 days)

**Goals:**
- Python is default
- Old code removed
- Documentation updated

**Tasks:**
1. Make Python default (remove feature flag)
2. Delete old `rig`-based code
3. Update README and docs
4. Update installation instructions
5. Clean up unused dependencies

**Success Criteria:**
- Clean codebase
- Clear documentation
- Easy to build and run

---

## Testing Strategy

### Unit Tests

**Python Side:**
```python
# tests/test_client.py
import pytest
from terminai_llm import LLMClient

@pytest.mark.asyncio
async def test_send_message():
    client = LLMClient("anthropic", "claude-sonnet-4-5")
    context = {"cwd": "/tmp", "history_lines": []}

    response_chunks = []
    async for chunk in client.send_message_stream(
        "Hello", context, []
    ):
        response_chunks.append(chunk)

    assert len(response_chunks) > 0
```

**Rust Side:**
```rust
#[tokio::test]
async fn test_python_bridge() {
    let bridge = LLMClientBridge::new(
        Provider::Anthropic,
        Some("claude-sonnet-4-5".to_string()),
        None,
    ).await.unwrap();

    let context = TerminalContext::empty(PathBuf::from("/tmp"));
    let mut stream = bridge.send_message_stream(
        "Hello",
        &context,
        &[],
    ).await.unwrap();

    let mut chunks = vec![];
    while let Some(chunk) = stream.next().await {
        chunks.push(chunk.unwrap());
    }

    assert!(!chunks.is_empty());
}
```

### Integration Tests

1. **End-to-end message flow**
2. **Tool calling roundtrip**
3. **Provider switching**
4. **Error handling**
5. **Streaming with backpressure**

### Performance Tests

Benchmark:
1. Message latency (time to first token)
2. Streaming throughput (tokens/sec)
3. Memory usage under load
4. GIL contention impact

Target: Within 10% of pure Rust implementation

---

## References

### Core Technologies

**PyO3 & Async Integration:**
- [PyO3 User Guide](https://pyo3.rs/) - Main documentation for Rust-Python bindings
- [pyo3-async-runtimes GitHub](https://github.com/PyO3/pyo3-async-runtimes) - Async bridge library
- [pyo3-async-runtimes Documentation](https://docs.rs/pyo3-async-runtimes/latest/pyo3_async_runtimes/)
- [Async / Await in PyO3](https://pyo3.rs/v0.13.2/ecosystem/async-await) - Async patterns guide
- [PyO3 Async Integration Patterns](https://github.com/PyO3/pyo3/discussions/3438) - Community discussions

**PydanticAI:**
- [PydanticAI Documentation](https://ai.pydantic.dev/) - Official docs
- [PydanticAI GitHub](https://github.com/pydantic/pydantic-ai) - Source code
- [PydanticAI Agents](https://ai.pydantic.dev/agents/) - Agent framework guide
- [PydanticAI Tools](https://ai.pydantic.dev/tools/) - Tool calling documentation
- [PydanticAI Output](https://ai.pydantic.dev/output/) - Structured outputs
- [Type-safe LLM agents with PydanticAI](https://simmering.dev/blog/pydantic-ai/) - Tutorial
- [Streaming with Pydantic AI](https://datastud.dev/posts/pydantic-ai-streaming/) - Streaming guide

**LiteLLM:**
- [LiteLLM Documentation](https://docs.litellm.ai/) - Main docs
- [LiteLLM GitHub](https://github.com/BerriAI/litellm) - Source code
- [LiteLLM Streaming + Async](https://docs.litellm.ai/docs/completion/stream) - Streaming guide
- [LiteLLM Integration for Pydantic AI](https://python.plainenglish.io/introducing-litellm-integration-for-pydantic-ai-659cd9e5753f) - Integration guide
- [pydantic-ai-litellm](https://github.com/mochow13/pydantic-ai-litellm) - Integration package

**LangChain (Alternative):**
- [LangChain Python Docs](https://python.langchain.com/)
- [LangChain Async Programming](https://python.langchain.com/docs/concepts/async/)
- [LangChain Async API](https://lagnchain.readthedocs.io/en/stable/modules/models/llms/examples/async_llm.html)

### Python Distribution & Packaging

**PyOxidizer:**
- [PyOxidizer GitHub](https://github.com/indygreg/PyOxidizer) - Main repository
- [PyOxidizer Documentation](https://pyoxidizer.readthedocs.io/en/stable/) - Official docs
- [Generic Python Embedding in Rust](https://pyoxidizer.readthedocs.io/en/stable/pyoxidizer_rust_generic_embedding.html) - Embedding guide
- [PyOxidizer Rust Projects](https://pyoxidizer.readthedocs.io/en/stable/pyoxidizer_rust_projects.html) - Integration patterns
- [PyOxidizer Comparisons](https://pyoxidizer.readthedocs.io/en/stable/pyoxidizer_comparisons.html) - vs other tools

**Nuitka:**
- [Nuitka GitHub](https://github.com/Nuitka/Nuitka) - Source code
- [Nuitka Documentation](https://nuitka.net/) - Official docs
- [Nuitka Use Cases](https://nuitka.net/user-documentation/use-cases.html) - Usage guide
- [Nuitka Bundle Standalone Binary](https://williamhuey.github.io/posts/nuitka-pyqtgraph-bundle-standalone-executable/) - Tutorial

**Maturin:**
- [Maturin User Guide](https://www.maturin.rs/) - Official docs
- [Maturin GitHub](https://github.com/PyO3/maturin) - Source code
- [Maturin Tutorial](https://www.maturin.rs/tutorial.html) - Getting started
- [PyO3 Building and Distribution](https://pyo3.rs/v0.27.2/building-and-distribution.html) - PyO3 + maturin

**Comparisons:**
- [Distribute Python applications - 7 best tools](https://www.augmentedmind.de/2021/05/16/distribute-python-applications/) - Tool comparison
- [4 Attempts at Packaging Python as an Executable](https://tryexceptpass.org/article/package-python-as-executable/) - Detailed comparison

### Code Examples & Tutorials

- [pyo3-async-runtimes examples](https://github.com/PyO3/pyo3-async-runtimes/blob/main/src/lib.rs) - Reference implementations
- [Embedding Python into Rust Hello World](https://github.com/indygreg/PyOxidizer/discussions/652) - PyOxidizer example
- [Building Asynchronous LLM Application](https://medium.com/@givkashi/building-an-asynchronous-llm-application-with-langchain-and-gpt-4o-mini-4ee0964d917c) - LangChain async patterns

---

## Open Questions

1. **Distribution Strategy:**
   - PyOxidizer for single binary?
   - Or require Python installation?
   - What about cross-compilation?

2. **Python Version:**
   - Minimum Python 3.9? 3.10?
   - How to handle version differences?

3. **Dependency Management:**
   - Bundle Python deps in binary?
   - Or use pip install at runtime?
   - Virtual environment handling?

4. **Performance Tuning:**
   - How much caching to add?
   - Connection pooling needed?
   - GIL optimization strategies?

5. **Error Recovery:**
   - What if Python crashes?
   - How to recover gracefully?
   - Logging strategy across languages?

---

## Next Steps

1. **Review this design** with team/stakeholders
2. **Decide on distribution strategy** (PyOxidizer vs system Python)
3. **Prototype Phase 0** (basic bridge) to validate approach
4. **Benchmark** a simple implementation to confirm performance is acceptable
5. **Get approval** to proceed with full implementation

---

## Appendix: Alternative Approaches Considered

### Alternative 1: Pure Rust with Better Library

**Approach:** Find or build better Rust LLM library

**Pros:**
- No Python dependency
- Simpler deployment

**Cons:**
- Rust LLM ecosystem still immature
- Would need to maintain ourselves
- Slower to get provider updates

**Verdict:** Rejected - Python ecosystem too valuable

### Alternative 2: Subprocess Communication

**Approach:** Run Python as subprocess, communicate via JSON

**Pros:**
- Simpler than PyO3
- Process isolation

**Cons:**
- Much higher latency
- Serialization overhead
- Complex state management
- No streaming support

**Verdict:** Rejected - performance unacceptable

### Alternative 3: HTTP Service

**Approach:** Run Python LLM service, call via HTTP

**Pros:**
- Clean separation
- Could be used by other tools

**Cons:**
- Complex deployment
- Network overhead
- Requires port management
- Not transparent to user

**Verdict:** Rejected - too complex for this use case

### Alternative 4: WASM

**Approach:** Compile Python to WASM, embed in Rust

**Pros:**
- Single binary
- Sandboxed

**Cons:**
- WASM Python is experimental
- Many libraries don't work
- Large binary size
- Complexity

**Verdict:** Rejected - too experimental

---

**Conclusion:** PyO3 with embedded Python provides the best balance of functionality, performance, and maintainability for our use case.
