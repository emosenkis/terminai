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
5. [Component Design](#component-design)
6. [Async Communication Bridge](#async-communication-bridge)
7. [Tool Calling Mechanism](#tool-calling-mechanism)
8. [API Design](#api-design)
9. [Error Handling](#error-handling)
10. [Migration Path](#migration-path)
11. [Trade-offs and Considerations](#trade-offs-and-considerations)
12. [Implementation Phases](#implementation-phases)
13. [Testing Strategy](#testing-strategy)
14. [References](#references)

---

## Overview

This document proposes replacing the current Rust-based LLM integration (using the `rig` library) with a Python-based solution that leverages Python's mature LLM ecosystem while maintaining tight integration with Termin.AI's Rust codebase through PyO3.

### Key Goals

- **Leverage Python's LLM Ecosystem**: Access to mature libraries like LiteLLM, LangChain, and OpenAI SDK
- **Maintain Performance**: Async communication between Rust and Python
- **Preserve Features**: Keep all existing functionality (streaming, tools, multi-provider support)
- **Improve Maintainability**: Simpler LLM integration code with better library support
- **Enable Future Extension**: Easier to add new providers, tools, and features

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

#### Option 1: LiteLLM (Recommended)

**Pros:**
- Unified interface to 100+ providers (OpenAI, Anthropic, Gemini, Ollama, etc.)
- Built-in retry/fallback logic
- Native async/streaming support
- Cost tracking and rate limiting
- Minimal abstraction overhead
- 8ms P95 latency at 1k RPS

**Cons:**
- Less opinionated than LangChain (need to build more ourselves)
- Tool calling interface varies by provider

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

#### Option 2: LangChain

**Pros:**
- Full-featured framework with abstractions for everything
- Strong tool/agent support
- Extensive ecosystem (callbacks, tracing, memory, etc.)
- Good documentation and examples

**Cons:**
- Higher abstraction overhead
- More complex API
- Potentially slower than LiteLLM
- May include features we don't need

**Recommendation:** Start with **LiteLLM** for simplicity and performance, with option to switch to LangChain later if we need advanced features.

### Rust Libraries

- **pyo3** `v0.22+`: Core Rust-Python bindings
- **pyo3-async-runtimes** `v0.22+`: Async bridge between Tokio and asyncio
- **tokio**: Rust async runtime (already in use)

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
import asyncio
from typing import AsyncIterator, List, Dict, Optional, Any
from litellm import acompletion
import json

class LLMClient:
    """Main LLM client using LiteLLM for multi-provider support."""

    def __init__(
        self,
        provider: str,
        model: Optional[str] = None,
        api_key: Optional[str] = None,
    ):
        self.provider = provider
        self.model = model or self._default_model(provider)
        self.api_key = api_key
        self.tool_registry = ToolRegistry()
        self._suggested_commands = []

        # Set API key if provided
        if api_key:
            self._set_api_key(api_key)

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
        import os
        key_mapping = {
            "anthropic": "ANTHROPIC_API_KEY",
            "openai": "OPENAI_API_KEY",
            "gemini": "GOOGLE_API_KEY",
        }
        if self.provider in key_mapping:
            os.environ[key_mapping[self.provider]] = api_key

    async def send_message_stream(
        self,
        user_message: str,
        context: Dict[str, Any],
        history: List[Dict[str, str]],
    ) -> AsyncIterator[str]:
        """Send a message and stream the response."""

        # Format context and build messages
        messages = self._build_messages(user_message, context, history)

        # Get tools from registry
        tools = self.tool_registry.get_tool_definitions()

        # Stream completion
        response = await acompletion(
            model=f"{self.provider}/{self.model}",
            messages=messages,
            tools=tools if tools else None,
            stream=True,
        )

        # Process stream
        async for chunk in response:
            # Handle text content
            if chunk.choices[0].delta.content:
                yield chunk.choices[0].delta.content

            # Handle tool calls
            if chunk.choices[0].delta.tool_calls:
                await self._handle_tool_calls(
                    chunk.choices[0].delta.tool_calls
                )

    async def _handle_tool_calls(self, tool_calls):
        """Handle tool calls from LLM."""
        for tool_call in tool_calls:
            tool_name = tool_call.function.name
            args = json.loads(tool_call.function.arguments)

            # Execute tool
            result = await self.tool_registry.execute_tool(
                tool_name, args
            )

            # Store results (e.g., suggested commands)
            if tool_name == "suggest_command":
                self._suggested_commands.append({
                    "command": args["command"],
                    "explanation": args["explanation"],
                    "raw": args.get("raw", False),
                })

    def _build_messages(
        self,
        user_message: str,
        context: Dict[str, Any],
        history: List[Dict[str, str]],
    ) -> List[Dict[str, str]]:
        """Build message list with system prompt, history, and context."""
        messages = [
            {
                "role": "system",
                "content": self._get_system_prompt(),
            }
        ]

        # Add conversation history
        messages.extend(history)

        # Add current message with context
        context_str = self._format_context(context)
        full_message = f"{context_str}\n\n{user_message}"
        messages.append({
            "role": "user",
            "content": full_message,
        })

        return messages

    def _get_system_prompt(self) -> str:
        """Get the system prompt for the AI assistant."""
        return """You are an AI assistant integrated into a terminal.
You can help users with commands, explain output, and suggest actions.

When suggesting commands, use the suggest_command tool."""

    def _format_context(self, context: Dict[str, Any]) -> str:
        """Format terminal context for the prompt."""
        parts = []

        if context.get("cwd"):
            parts.append(f"Current directory: {context['cwd']}")

        if context.get("last_exit_code") is not None:
            parts.append(f"Last exit code: {context['last_exit_code']}")

        if context.get("history_lines"):
            history = "\n".join(context["history_lines"][-50:])  # Last 50 lines
            parts.append(f"Recent terminal output:\n```\n{history}\n```")

        return "\n".join(parts)

    def take_suggested_commands(self) -> List[Dict[str, Any]]:
        """Get and clear suggested commands."""
        commands = self._suggested_commands.copy()
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
- [ ] Implement basic LiteLLM client
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
   - LiteLLM supports 100+ providers vs ~5 in rig
   - Updates quickly when providers add features
   - Built-in retry, fallback, caching

2. **Easier Maintenance:**
   - Simpler code for LLM interactions
   - More examples and documentation
   - Larger community for support

3. **Future Flexibility:**
   - Easy to add new providers
   - Access to Python AI ecosystem (RAG, agents, etc.)
   - Can leverage Python-only features quickly

4. **Development Speed:**
   - Faster iteration on LLM features
   - Better debugging tools
   - Rich ecosystem for observability

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

### Documentation

- [PyO3 User Guide](https://pyo3.rs/)
- [pyo3-async-runtimes](https://github.com/PyO3/pyo3-async-runtimes)
- [LiteLLM Documentation](https://docs.litellm.ai/)
- [LangChain Python Docs](https://python.langchain.com/)
- [Async / Await in PyO3](https://pyo3.rs/v0.13.2/ecosystem/async-await)

### Blog Posts & Tutorials

- [PyO3 Async Integration Patterns](https://github.com/PyO3/pyo3/discussions/3438)
- [LiteLLM Streaming Guide](https://docs.litellm.ai/docs/completion/stream)
- [Building Async Python Libraries](https://python.langchain.com/docs/concepts/async/)

### Code Examples

- [pyo3-async-runtimes examples](https://github.com/PyO3/pyo3-async-runtimes/tree/main/examples)
- [LiteLLM async examples](https://github.com/BerriAI/litellm/tree/main/examples)

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
