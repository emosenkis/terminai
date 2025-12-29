# LLM Integration Architecture

**Created:** 2025-12-29
**Status:** Current Implementation (Post-Migration)
**Migration:** Rust (rig-core) → Python (Pydantic AI) ✅ Complete

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture Diagram](#architecture-diagram)
3. [Component Breakdown](#component-breakdown)
4. [Communication Protocol](#communication-protocol)
5. [Tool Execution Flow](#tool-execution-flow)
6. [Provider Configuration](#provider-configuration)
7. [Design Decisions](#design-decisions)
8. [Testing Strategy](#testing-strategy)

---

## Overview

Termin.AI integrates LLM (Large Language Model) assistance using a **Rust + Python hybrid architecture**:

- **Rust (termin)**: Terminal emulation, UI, tool execution, subprocess management
- **Python (terminai_agent)**: LLM API calls, agent logic, conversation management

This split leverages each language's strengths:
- Rust: Performance, safety, terminal handling
- Python: Rich LLM ecosystem, Pydantic AI framework

### Key Technologies

- **Rust Side:**
  - [ag-ui-client](https://github.com/pydantic/pydantic-ai): Official Rust client for AG-UI protocol
  - [tokio](https://tokio.rs/): Async runtime
  - Custom subprocess management

- **Python Side:**
  - [Pydantic AI](https://ai.pydantic.dev/): Agent framework with tool support
  - [AG-UI Protocol](https://ai.pydantic.dev/ui/ag-ui/): Standardized agent communication
  - Multiple LLM providers (Anthropic, OpenAI, Gemini, Ollama, OpenRouter)

---

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                        Termin.AI (Rust)                         │
│                                                                   │
│  ┌────────────────┐                    ┌─────────────────────┐  │
│  │  Terminal UI   │                    │  Tool Coordinator   │  │
│  │   (Ratatui)    │                    │                     │  │
│  └────────┬───────┘                    │  • suggest_command  │  │
│           │                             │  • read_scrollback  │  │
│  ┌────────▼───────┐                    └─────────┬───────────┘  │
│  │   AI Overlay   │                              │              │
│  │                │                              │              │
│  │  • Chat UI     │                              │              │
│  │  • Message     │         ┌────────────────────▼───────────┐  │
│  │    History     │         │     AG-UI Client (Rust)        │  │
│  └────────┬───────┘         │                                │  │
│           │                 │  • Spawn Python subprocess     │  │
│           │                 │  • Send/receive AG-UI events   │  │
│           └─────────────────►  • Stream responses            │  │
│                             │  • Execute client-side tools   │  │
│                             └─────────────┬──────────────────┘  │
└─────────────────────────────────────────┼─────────────────────┘
                                          │
                         AG-UI Protocol (stdio)
                         JSON-RPC over stdin/stdout
                                          │
┌─────────────────────────────────────────▼─────────────────────┐
│                   terminai_agent (Python)                       │
│                                                                   │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │              Pydantic AI Agent                           │  │
│  │                                                          │  │
│  │  System Prompt + Model + Tools                          │  │
│  │                                                          │  │
│  │  Server-side Tools:           Client-side Tools:        │  │
│  │  • read_file_tool             • suggest_command (Rust)  │  │
│  │  • grep_files_tool            • read_scrollback (Rust)  │  │
│  │                                                          │  │
│  └─────────────────────────┬────────────────────────────────┘  │
│                            │                                     │
│  ┌─────────────────────────▼────────────────────────────────┐  │
│  │           Provider Adapters                              │  │
│  │                                                          │  │
│  │  • Anthropic (Claude)  • OpenAI (GPT)  • Gemini        │  │
│  │  • Ollama (local)      • OpenRouter (aggregator)       │  │
│  └─────────────────────────┬────────────────────────────────┘  │
│                            │                                     │
└────────────────────────────┼───────────────────────────────────┘
                             │
                    API Calls (HTTPS)
                             │
                 ┌───────────▼────────────┐
                 │   LLM Provider APIs    │
                 │                        │
                 │ • claude.ai            │
                 │ • api.openai.com       │
                 │ • generativelanguage   │
                 │ • localhost:11434      │
                 └────────────────────────┘
```

---

## Component Breakdown

### Rust Components

#### 1. `llm::AgUiClient`
**Location:** `src/llm/client.rs`

**Purpose:** Manages Python subprocess and AG-UI communication

**Responsibilities:**
- Spawn Python subprocess (`python -m terminai_agent`)
- Send RunAgent requests via AG-UI protocol
- Stream responses back to UI
- Handle subprocess lifecycle (start/stop/crash recovery)

**Key APIs:**
```rust
pub struct AgUiClient {
    // Internal: subprocess, transport, etc.
}

impl AgUiClient {
    pub async fn spawn(config, provider, model) -> Result<Self>;
    pub async fn chat_stream(&self, message, context, history) -> ChatStreamResponse;
    pub async fn shutdown(&self) -> Result<()>;
}
```

#### 2. `llm::ToolCoordinator`
**Location:** `src/llm/tool_coordinator.rs`

**Purpose:** Coordinate client-side tool execution

**Responsibilities:**
- Receive tool requests from Python (via AG-UI protocol)
- Execute Rust-side tools (`suggest_command`, `read_scrollback`)
- Send tool results back to Python
- Handle concurrent tool executions

**Key APIs:**
```rust
pub struct ToolCoordinator {
    // Internal: executor, event channels, etc.
}

impl ToolCoordinator {
    pub fn new(executor: ToolExecutor) -> Self;
    pub async fn handle_tool_request(&self, request: ToolRequest) -> ToolResult;
}
```

#### 3. `llm::ToolExecutor`
**Location:** `src/llm/tool_executor.rs`

**Purpose:** Execute individual tools

**Responsibilities:**
- `suggest_command`: Validate and suggest shell commands
- `read_scrollback`: Extract terminal history for AI context

**Key APIs:**
```rust
pub struct ToolExecutor {
    pub command_executor: CommandExecutor,
    pub safety_validator: SafetyValidator,
    pub vt_parser: Option<Arc<RwLock<Parser>>>,
}

impl ToolExecutor {
    pub async fn execute(&self, name: &str, args: Value) -> ToolResult;
}
```

#### 4. `llm::TerminalContext`
**Location:** `src/llm/terminal_context.rs`

**Purpose:** Terminal state snapshot for AI

**Structure:**
```rust
pub struct TerminalContext {
    pub history_lines: Vec<String>,  // Recent terminal output
    pub cwd: String,                  // Current working directory
    pub last_exit_code: Option<i32>, // Last command result
}
```

**Conversion:** `to_ag_ui_context()` → AG-UI Context items

---

### Python Components

#### 1. `terminai_agent.TerminAIAgent`
**Location:** `python/terminai_agent/agent.py`

**Purpose:** Main AI agent using Pydantic AI

**Responsibilities:**
- Create Pydantic AI Agent with system prompt
- Register server-side tools (`read_file_tool`, `grep_files_tool`)
- Handle chat requests from Rust
- Stream responses back via AG-UI protocol

**Key APIs:**
```python
class TerminAIAgent:
    def __init__(self, provider_config: ProviderConfig):
        self.agent = self._create_agent()

    async def chat(self, user_message: str, context: TerminalContext,
                   history: list[Message]) -> str:
        # Run agent and return response

    async def chat_stream(self, ...) -> AsyncIterator[str]:
        # Stream response chunks
```

#### 2. `terminai_agent.tools`
**Location:** `python/terminai_agent/tools/`

**Purpose:** Server-side (Python) tool implementations

**Tools:**
- **`read_file_tool`**: Read file contents with optional line ranges
  - Safety: Path traversal prevention, file size limits
  - Returns: Formatted file content or error

- **`grep_files_tool`**: Search files for patterns
  - Features: Regex support, glob patterns, case-insensitive
  - Safety: Skip binary files, hidden dirs, large files
  - Returns: Formatted search results

**Registration:**
```python
@agent.tool
async def read_file_tool(ctx: RunContext[TerminalContext],
                         path: str, ...) -> str:
    result = await read_file(args, cwd=ctx.deps.cwd)
    # Return formatted content or raise error
```

#### 3. `terminai_agent.config`
**Location:** `python/terminai_agent/config.py`

**Purpose:** Provider configuration and validation

**Provider Enum (Python):**
```python
class Provider(str, Enum):
    ANTHROPIC = "anthropic"
    OPENAI = "openai"
    GEMINI = "gemini"
    OLLAMA = "ollama"
    OPENROUTER = "openrouter"
```

**Note:** Duplicated from Rust - see [Provider Duplication](#provider-duplication)

---

## Communication Protocol

### AG-UI Protocol

**Transport:** JSON-RPC over stdin/stdout

**Message Types:**

1. **RunAgent Request** (Rust → Python)
   ```json
   {
     "method": "run_agent",
     "params": {
       "message": "How do I list all Python files?",
       "context": [
         {"description": "Current working directory", "value": "/home/user/project"},
         {"description": "Recent terminal history", "value": "..."}
       ],
       "tools": [
         {"name": "suggest_command", "description": "...", "parameters": {...}},
         {"name": "read_scrollback", "description": "...", "parameters": {...}}
       ]
     }
   }
   ```

2. **StreamText Event** (Python → Rust)
   ```json
   {
     "type": "stream_text",
     "data": {"text": "You can use the command..."}
   }
   ```

3. **ToolCall Event** (Python → Rust)
   ```json
   {
     "type": "tool_call",
     "data": {
       "tool_name": "suggest_command",
       "args": {"command": "find . -name '*.py'"}
     }
   }
   ```

4. **ToolResult Event** (Rust → Python)
   ```json
   {
     "type": "tool_result",
     "data": {
       "tool_call_id": "123",
       "result": "Command validated successfully"
     }
   }
   ```

### Event Flow Diagram

```
Rust                    Python
 │                        │
 ├─ RunAgent Request ────►│
 │                        │
 │                        ├─ Process with LLM
 │                        │
 │◄─── StreamText ────────┤ (streaming response)
 │◄─── StreamText ────────┤
 │                        │
 │◄─── ToolCall ──────────┤ (wants to use Rust tool)
 │                        │
 ├─ Execute Tool          │
 │                        │
 ├─ ToolResult ──────────►│
 │                        │
 │                        ├─ Continue with LLM
 │                        │
 │◄─── StreamText ────────┤
 │◄─── Complete ──────────┤
 │                        │
```

---

## Tool Execution Flow

### Client-Side Tools (Rust)

**Purpose:** Access terminal-specific state that Python can't see

**Examples:**
- `suggest_command`: Validate command against safety rules, terminal state
- `read_scrollback`: Extract VT100 buffer content

**Flow:**
1. Python agent decides to use tool (via LLM)
2. Sends ToolCall event to Rust
3. Rust executes tool with full access to terminal state
4. Rust sends ToolResult back to Python
5. Python incorporates result into conversation
6. LLM generates response using tool result

### Server-Side Tools (Python)

**Purpose:** Filesystem operations that don't need terminal state

**Examples:**
- `read_file_tool`: Read source code files
- `grep_files_tool`: Search for patterns in codebase

**Flow:**
1. Python agent decides to use tool (via LLM)
2. Executes tool directly (synchronous to agent)
3. Tool result incorporated into LLM context
4. LLM generates response using tool result
5. Response streamed to Rust

### Why Split Tools?

| Aspect | Rust Tools | Python Tools |
|--------|------------|--------------|
| **Access** | Terminal state, VT100 buffer, process info | Filesystem (relative to cwd) |
| **Latency** | Low (no IPC) for Rust, high (IPC) for Python | Low (in-process) |
| **Use Case** | Terminal-specific, real-time state | Generic file operations |
| **Examples** | suggest_command, read_scrollback | read_file, grep_files |

**Trade-off:** We could implement all tools in Rust (lower latency) but then Python agent needs more complex IPC for every tool. Current split balances complexity and performance.

---

## Provider Configuration

### Provider Enum Duplication

The `Provider` enum is **intentionally duplicated** between Rust and Python:

**Rust** (`src/llm/providers.rs`):
```rust
pub enum Provider {
    Anthropic, OpenAI, Gemini, Ollama, OpenRouter
}

impl Provider {
    pub fn api_key_env(&self) -> Option<&str> {
        // Used for pre-flight validation
    }
}
```

**Python** (`python/terminai_agent/config.py`):
```python
class Provider(str, Enum):
    ANTHROPIC = "anthropic"
    OPENAI = "openai"
    # ...

class ProviderConfig:
    def from_env(provider: Provider) -> ProviderConfig:
        # Actual provider selection and API key loading
```

### Why Duplicate?

**Rust Side:**
- **Pre-flight validation**: Check API keys before spawning subprocess
- **User feedback**: Show warnings immediately if API key missing
- **Fast fail**: Don't waste time starting Python if config is invalid

**Python Side:**
- **Actual implementation**: Select provider, configure API client
- **Runtime behavior**: Handle API calls, retries, errors

### UX Impact

```
WITH Rust validation:
$ terminai
⚠️  Warning: ANTHROPIC_API_KEY not set
[Continue? Y/n]

WITHOUT Rust validation:
$ terminai
[loads...]
[spawns Python...]
❌ Error: ANTHROPIC_API_KEY environment variable required
```

**Verdict:** Duplication provides better UX. Small maintenance cost (keep in sync when adding providers).

---

## Design Decisions

### 1. Why Python subprocess vs. Rust-only?

**Considered:**
- ✅ Rust + Python hybrid (chosen)
- ❌ Pure Rust with native LLM clients
- ❌ Pure Python for entire app

**Rationale:**
- **Ecosystem**: Python has mature LLM libraries (Pydantic AI, LangChain, etc.)
- **Velocity**: Faster iteration on AI features
- **Type Safety**: Pydantic provides excellent schema validation
- **Performance**: Terminal emulation in Rust keeps UI responsive
- **Trade-off**: IPC overhead acceptable (chat is not latency-critical)

### 2. Why AG-UI protocol?

**Considered:**
- ✅ AG-UI (chosen)
- ❌ Custom JSON-RPC protocol
- ❌ HTTP server in Python

**Rationale:**
- **Standard**: Official Pydantic AI UI protocol
- **Tools**: Bidirectional tool execution built-in
- **Streaming**: Native support for streaming responses
- **Maintenance**: Official Rust client (`ag-ui-client`) maintained by Pydantic
- **Testing**: Can use official AG-UI test utilities

### 3. Why subprocess vs. embedded Python?

**Considered:**
- ✅ Subprocess (chosen)
- ❌ PyO3 embedded Python
- ❌ Python as parent process

**Rationale:**
- **Isolation**: Crash in AI logic doesn't kill terminal
- **Simplicity**: No FFI complexity, cleaner boundaries
- **Compatibility**: Works across Python versions
- **Distribution**: No need to bundle Python runtime
- **Trade-off**: Higher startup latency (acceptable for chat)

### 4. Tool split: Why not all in Rust?

**Rationale:**
- **Cohesion**: File tools live with AI logic (Python)
- **Iteration speed**: Easier to add new Python tools
- **Context**: Python tools can access `ctx.deps.cwd` naturally
- **Trade-off**: Slight latency for Rust tools due to IPC

---

## Testing Strategy

### Unit Tests

**Rust:**
- `llm::client::tests::test_client_lifecycle` - Subprocess spawn/shutdown
- `command::parser::tests::*` - Command parsing
- `privacy::filter::tests::*` - Secret redaction

**Python:**
- `tests/test_agent.py` - Agent creation, context building
- `tests/test_tools.py` - File tools (read_file, grep_files)

### Integration Tests

**Rust E2E:**
- `tests/test_llm_e2e.rs` - Full AG-UI protocol with mock Python server
- Verifies: Tool calls, streaming, error handling

**Python E2E:**
- Not yet implemented (see TODO.md #14)
- Needed: Tests with real LLM calling tools

### Mock Architecture

```rust
// test_llm_e2e.rs
MockLLMServer::new()
    .expect_run_agent()
    .respond_with_stream(vec!["Hello", " world"])
    .expect_tool_call("suggest_command", args)
    .respond_with_result(result);
```

### Test Isolation

- **Rust tests**: Use mock Python server (no real subprocess)
- **Python tests**: Use in-memory filesystems for file tools
- **E2E tests**: Can use Ollama with feature flag (`--features ollama-tests`)

---

## Troubleshooting

### Common Issues

**1. Python subprocess fails to start**
- Check Python installation: `python3 -m terminai_agent --version`
- Check dependencies: `cd python && uv sync`
- Check logs: Look for subprocess stderr in Rust logs

**2. Tool calls hang**
- Check tool coordinator event loop is running
- Verify tool registration in Python (`agent._function_tools`)
- Check AG-UI protocol logs for errors

**3. Streaming stops mid-response**
- Check network/API errors in Python logs
- Verify API rate limits not exceeded
- Check subprocess didn't crash (Rust should log)

### Debug Logging

**Rust:**
```bash
RUST_LOG=debug cargo run
```

**Python:**
```bash
PYTHON_LOG_LEVEL=DEBUG python -m terminai_agent
```

---

## Future Improvements

See `TODO.md` for tracked items:

- [ ] E2E tests with Python tools (#14)
- [ ] Architecture diagram in docs
- [ ] Performance profiling (tool call latency)
- [ ] Support for more LLM providers
- [ ] Conversation history persistence
- [ ] Multi-turn tool usage optimization

---

## References

- [Pydantic AI Documentation](https://ai.pydantic.dev/)
- [AG-UI Protocol Spec](https://ai.pydantic.dev/ui/ag-ui/)
- [Original Design Doc](../LLM_OVER_AG_UI_DESIGN.md)
- [Migration Progress](../LLM_OVER_AG_UI_PROGRESS.md)
- [Cleanup Action Plan](../CLEANUP_ACTION_PLAN_V2.md)

---

**Last Updated:** 2025-12-29
**Maintained By:** See git history
**Questions?** Check `CLAUDE.md` for development guidelines
