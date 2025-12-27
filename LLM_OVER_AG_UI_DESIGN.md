# LLM over AG-UI Architecture Design

**Date:** 2025-12-27
**Status:** Design Phase

## Overview

This document describes the architecture for refactoring Termin.AI's LLM integration from a Rust-native implementation to a Python subprocess using the AG-UI protocol.

## Current Architecture

### Components
- **LLM Client** (`src/llm/client.rs`): Rust-native LLM client using rig-core
  - Supports multiple providers: Anthropic, OpenAI, Gemini, Ollama, OpenRouter
  - Streaming and non-streaming responses
  - Tool support via rig's agent framework

- **Tools** (`src/llm/tools/`):
  - `suggest_command` - Suggests shell commands for execution
  - `read_scrollback` - Reads terminal scrollback history
  - `read_file` - Reads file contents from disk
  - `grep_files` - Searches files using regex patterns

- **AI Chat Process** (`src/ai_proc/chat_process.rs`):
  - Manages conversation state
  - Handles command approval workflow
  - Integrates with privacy filter and safety validator
  - Supports streaming responses

- **Supporting Components**:
  - Command parser (`src/command/parser.rs`)
  - Safety validator (`src/command/validator.rs`)
  - Privacy filter (referenced but details TBD)

### Current Flow
```
User Input → AIChatProcess → LLMClient (Rust) → rig-core → Provider API
                    ↓
              Tool Execution (Rust)
                    ↓
              Response Processing → UI Rendering
```

## New Architecture

### High-Level Design

The new architecture moves LLM orchestration to a Python subprocess while keeping terminal-specific operations in Rust:

```
┌─────────────────────────────────────────────────────────────┐
│                     Termin.AI (Rust)                        │
│                                                             │
│  ┌──────────────────────────────────────────────────────┐  │
│  │              AI Chat Process                          │  │
│  │  - Conversation state                                 │  │
│  │  - Command approval workflow                          │  │
│  │  - UI integration                                     │  │
│  └─────────────────┬────────────────────────────────────┘  │
│                    │                                        │
│  ┌─────────────────▼────────────────────────────────────┐  │
│  │         AG-UI Client (ag-ui-client crate)            │  │
│  │  - HTTP client to Python subprocess                  │  │
│  │  - Tool registration & execution                     │  │
│  │  - Shared secret authentication                      │  │
│  └─────────────────┬────────────────────────────────────┘  │
│                    │                                        │
│  ┌─────────────────▼────────────────────────────────────┐  │
│  │            Terminal Tools (Rust)                     │  │
│  │  - suggest_command                                   │  │
│  │  - read_scrollback                                   │  │
│  │  - send_keys (future)                                │  │
│  │  - get_terminal_state (future)                       │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                             │
                             │ HTTP/AG-UI Protocol
                             │
┌─────────────────────────────▼───────────────────────────────┐
│              Python Subprocess (uv run)                     │
│                                                             │
│  ┌──────────────────────────────────────────────────────┐  │
│  │         AG-UI Server (FastAPI + Pydantic AI)         │  │
│  │  - Listens on auto-selected port                     │  │
│  │  - Verifies shared secret                            │  │
│  │  - Serves AG-UI protocol                             │  │
│  └─────────────────┬────────────────────────────────────┘  │
│                    │                                        │
│  ┌─────────────────▼────────────────────────────────────┐  │
│  │         Pydantic AI Agent                            │  │
│  │  - LLM provider clients (Anthropic, OpenAI, etc.)   │  │
│  │  - Conversation management                           │  │
│  │  - Tool calling orchestration                        │  │
│  │  - Streaming response handling                       │  │
│  └─────────────────┬────────────────────────────────────┘  │
│                    │                                        │
│  ┌─────────────────▼────────────────────────────────────┐  │
│  │              Python-Side Tools                       │  │
│  │  - read_file                                         │  │
│  │  - grep_files                                        │  │
│  │  - list_files (future)                               │  │
│  │  - analyze_diff (future)                             │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### Component Details

#### 1. Python Subprocess Lifecycle

**Startup Sequence:**
1. Rust main process generates a random shared secret (UUID)
2. Spawns Python subprocess: `uv run python -m terminai_agent --secret <UUID>`
3. Python process:
   - Selects available port (try 18080-18099, fallback to OS-assigned)
   - Outputs to stdout: `AG_UI_PORT=<port>\n`
   - Outputs to stderr: logging information
4. Rust reads stdout line-by-line until it gets `AG_UI_PORT=<port>`
5. Rust connects AG-UI client to `http://localhost:<port>` with secret in header

**Stderr Monitoring:**
- Rust spawns async task to read stderr continuously
- Logs Python output using Rust's `log` crate with `[Python]` prefix
- Monitors for Python crashes/exits

**Shutdown:**
- Rust sends shutdown signal to Python subprocess
- Waits up to 5 seconds for graceful shutdown
- Force-kills if necessary

#### 2. AG-UI Protocol Integration

**Rust Side (ag-ui-client):**
```rust
// Client configuration
struct AgUiClient {
    base_url: String,  // http://localhost:<port>
    secret: String,
    http_client: reqwest::Client,
}

// Tool registration
impl AgUiClient {
    async fn register_tool(&self, tool: &dyn Tool) -> Result<()>;
    async fn chat_stream(&self, message: &str, context: Context)
        -> Result<impl Stream<Item=Result<ChatToken>>>;
}
```

**Python Side (FastAPI server):**
```python
from fastapi import FastAPI, Header, HTTPException
from pydantic_ai import Agent
from ag_ui_protocol import serve_agent

app = FastAPI()
agent = Agent(...)  # Pydantic AI agent

@app.middleware("http")
async def verify_secret(request: Request, call_next):
    secret = request.headers.get("X-AG-UI-Secret")
    if secret != EXPECTED_SECRET:
        raise HTTPException(401, "Invalid secret")
    return await call_next(request)

# AG-UI protocol endpoints provided by library
serve_agent(app, agent)
```

#### 3. Tool Distribution

**Terminal Tools (Rust):**
These tools directly interact with the terminal emulator state:
- `suggest_command(command: str, explanation: str, raw: bool)` - Suggests command for approval
- `read_scrollback(lines: int)` - Reads terminal scrollback buffer
- Future: `send_keys`, `get_cursor_position`, `get_terminal_dimensions`

**File Tools (Python):**
These tools work with the filesystem and are easier to implement in Python:
- `read_file(path: str, start_line: int, max_lines: int)` - Reads file contents
- `grep_files(pattern: str, file_pattern: str, case_insensitive: bool)` - Searches files
- Future: `list_files`, `analyze_diff`, `git_status`

**Tool Calling Flow:**
```
1. User sends message to Rust
2. Rust forwards to Python via AG-UI
3. Python LLM decides to call tool:
   - If Rust tool: Python makes HTTP request to Rust endpoint
   - If Python tool: Python executes directly
4. Tool result returned to LLM
5. LLM generates response
6. Response streamed back to Rust → UI
```

#### 4. Data Structures

**Terminal Context:**
```rust
// Rust side
pub struct TerminalContext {
    pub history_lines: Vec<String>,
    pub cwd: PathBuf,
    pub last_exit_code: Option<i32>,
}

// Serialized to JSON for Python
{
    "history_lines": ["$ ls", "file.txt", "$ cat file.txt", ...],
    "cwd": "/home/user/project",
    "last_exit_code": 0
}
```

**Suggested Command:**
```rust
pub struct SuggestedCommand {
    pub command: String,
    pub explanation: String,
    pub raw: bool,  // Contains escape sequences
}
```

**Chat Message:**
```rust
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
}

pub enum MessageRole {
    User,
    Assistant,
    System,
}
```

#### 5. Configuration

**Python Dependencies (pyproject.toml managed by uv):**
```toml
[project]
name = "terminai-agent"
version = "0.1.0"
dependencies = [
    "pydantic-ai[anthropic,openai,gemini]",
    "fastapi",
    "uvicorn",
    "httpx",
]
```

**Provider Configuration:**
Reuse existing `terminai_config.rs` system:
- Python reads environment variables for API keys
- Same provider names: anthropic, openai, gemini, ollama, openrouter
- Model names passed from Rust config to Python

## Implementation Plan

### Phase 1: Python Subprocess Foundation
1. Create Python project structure with uv
2. Implement basic FastAPI server with AG-UI endpoints
3. Implement subprocess launch and port discovery in Rust
4. Verify HTTP communication works
5. Add shared secret authentication

**Deliverables:**
- `python/terminai_agent/` directory with uv project
- `src/llm_subprocess.rs` - subprocess management
- Basic health check endpoint working

### Phase 2: Pydantic AI Integration
1. Set up Pydantic AI agent with provider support
2. Implement conversation management
3. Implement streaming responses
4. Add basic error handling

**Deliverables:**
- Working Pydantic AI agent in Python
- Streaming chat endpoint
- Provider configuration support

### Phase 3: Tool Migration - Python Side
1. Implement `read_file` tool in Python
2. Implement `grep_files` tool in Python
3. Register tools with Pydantic AI agent
4. Write unit tests for Python tools

**Deliverables:**
- `python/terminai_agent/tools/` module
- Working file tools in Python
- Unit tests with pytest

### Phase 4: Tool Migration - Rust Side
1. Implement `suggest_command` as AG-UI tool in Rust
2. Implement `read_scrollback` as AG-UI tool in Rust
3. Register Rust tools with Python agent via AG-UI
4. Test bidirectional tool calling

**Deliverables:**
- `src/llm/ag_ui_tools/` module
- Terminal tools exposed via AG-UI
- Integration tests

### Phase 5: AI Chat Process Refactoring
1. Replace LLMClient with AgUiClient in AIChatProcess
2. Update streaming response handling
3. Update conversation management
4. Preserve command approval workflow
5. Update error handling

**Deliverables:**
- Updated `src/ai_proc/chat_process.rs`
- All existing features working via AG-UI
- No regressions in UI behavior

### Phase 6: Cleanup and Testing
1. Remove old `src/llm/client.rs` and provider code
2. Remove rig-core dependency
3. Update Cargo.toml dependencies
4. Write integration tests
5. Update documentation

**Deliverables:**
- Clean codebase with no dead code
- Full test coverage
- Updated README and docs

## Design Decisions

### Why Python subprocess vs. Rust?
**Pros:**
- Pydantic AI is more mature than rig for agent orchestration
- Python has better LLM provider libraries (official SDKs)
- Easier to iterate on prompt engineering in Python
- Better ecosystem for AI/ML tooling

**Cons:**
- Additional subprocess complexity
- Inter-process communication overhead
- Two language maintenance burden

**Decision:** Benefits outweigh costs. Terminal-specific code stays in Rust where it belongs.

### Why AG-UI protocol?
- Standard protocol for agent communication
- Built-in tool calling support
- Already has Rust client library
- Language-agnostic design
- Future-proof for multi-agent systems

### Tool Distribution Strategy
**Terminal tools → Rust:**
- Must access VT100 screen state
- Must access terminal emulator directly
- Performance-critical (scrollback can be large)

**File tools → Python:**
- Filesystem operations are cross-platform
- Python has great file handling libraries
- Not performance-critical
- Easier to extend (git operations, etc.)

### Process Communication
- **Port selection:** Auto-select to avoid conflicts
- **Shared secret:** UUID prevents unauthorized access
- **Stderr monitoring:** Critical for debugging and error reporting
- **Graceful shutdown:** Clean resource cleanup

## Security Considerations

1. **Shared Secret:** Must be cryptographically random (UUID v4)
2. **Port Binding:** Bind to 127.0.0.1 only (no network access)
3. **Path Traversal:** Python tools must validate file paths
4. **Command Injection:** Preserve existing safety validator
5. **API Keys:** Keep in environment, never log or transmit

## Performance Considerations

1. **Latency:**
   - HTTP overhead: ~1-2ms on localhost
   - Python startup: ~100-300ms (uv is fast)
   - Overall impact: minimal compared to LLM API latency

2. **Memory:**
   - Python process: ~50-100MB baseline
   - Acceptable for terminal application

3. **Streaming:**
   - HTTP chunked transfer encoding for streaming responses
   - No buffering delays

## Testing Strategy

### Unit Tests

**Python:**
```bash
cd python/terminai_agent
uv run pytest tests/
```
- Test each tool independently
- Mock Pydantic AI agent
- Test error conditions

**Rust:**
```bash
cargo test --lib
```
- Test AG-UI client
- Test tool registration
- Test subprocess management

### Integration Tests
```bash
cargo test --test integration_llm_subprocess
```
- Full subprocess lifecycle
- Tool calling round-trips
- Streaming responses
- Error recovery

### Manual Testing
- Test with each provider (Anthropic, OpenAI, etc.)
- Test command suggestions and approval flow
- Test file reading tools
- Test grep functionality
- Test error scenarios (Python crash, network issues)

## Migration Path

1. **Parallel Implementation:** Build new system alongside old
2. **Feature Flag:** Use environment variable to switch between old/new
3. **Gradual Migration:** Test each component individually
4. **Deprecation:** Remove old code only after full validation
5. **Rollback Plan:** Keep old code in git history for quick revert

## Open Questions

1. ~~How to handle Python dependencies on different platforms?~~
   → uv handles cross-platform dependencies automatically

2. ~~Should we support multiple Python versions?~~
   → Require Python 3.11+ (type hints with `|` syntax)

3. ~~How to package Python code for distribution?~~
   → Include `python/` directory in repo, users need uv installed

4. ~~Should we cache Python subprocess or restart per query?~~
   → Long-lived subprocess, restart only on crash

5. ~~How to handle provider API key rotation?~~
   → Subprocess restart required (acceptable tradeoff)

## Success Criteria

- [ ] Python subprocess starts reliably across platforms
- [ ] AG-UI communication works bidirectionally
- [ ] All existing tools work via new architecture
- [ ] Streaming responses work smoothly
- [ ] Command approval workflow unchanged
- [ ] No performance regressions (subjective latency)
- [ ] Error messages are clear and actionable
- [ ] All unit tests passing
- [ ] Integration tests covering main flows
- [ ] Documentation updated
- [ ] Old Rust LLM code removed
- [ ] Clean `cargo build` and `cargo test`

## Future Enhancements

1. **Multi-Agent Support:** AG-UI protocol supports multiple agents
2. **Agent Plugins:** Users can add custom Python tools
3. **Conversation Persistence:** Save/load conversations in Python
4. **RAG Integration:** Add vector store for documentation search
5. **Code Analysis Tools:** Advanced Python tools for code understanding
6. **Parallel Tool Execution:** AG-UI supports concurrent tool calls

## References

- [AG-UI Protocol Specification](https://github.com/ag-ui-protocol/ag-ui)
- [Pydantic AI Documentation](https://ai.pydantic.dev/)
- [ag-ui-client Rust SDK](https://github.com/ag-ui-protocol/ag-ui/tree/main/sdks/community/rust/crates/ag-ui-client)
- [FastAPI Documentation](https://fastapi.tiangolo.com/)
- [uv Package Manager](https://github.com/astral-sh/uv)

---

**Next Steps:** Review design with stakeholders → Begin Phase 1 implementation
