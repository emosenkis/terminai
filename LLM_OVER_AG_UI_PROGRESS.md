# LLM over AG-UI Implementation Progress

**Last Updated:** 2025-12-27
**Status:** In Progress

## Implementation Phases

### ✅ Phase 0: Exploration and Design
- [x] Explored current Rust LLM implementation
- [x] Identified current tools and their purposes
- [x] Analyzed current architecture and data flow
- [x] Wrote comprehensive design document
- [x] Created progress tracking document

**Decisions Made:**
- Tool distribution: Terminal tools (suggest_command, read_scrollback) in Rust; File tools (read_file, grep_files) in Python
- Communication: AG-UI protocol over HTTP with shared secret authentication
- Python runtime: uv for dependency management
- Port selection: Auto-select from range, communicate via stdout

### ✅ Phase 1: Python Subprocess Foundation
**Status:** Completed

**Tasks:**
- [x] Create Python project structure with uv (`python/terminai_agent/`)
- [x] Set up pyproject.toml with dependencies
- [x] Create basic FastAPI server
- [x] Implement health check endpoint
- [x] Implement subprocess launch in Rust (`src/llm_subprocess.rs`)
- [x] Implement port discovery (stdout parsing)
- [x] Implement stderr monitoring and logging
- [x] Add shared secret generation and verification
- [x] Test basic HTTP communication

**Implementation Notes:**
- Python project uses uv for dependency management
- FastAPI server with middleware for secret verification
- Port auto-discovery in range 18080-18099 with OS fallback
- Subprocess outputs `AG_UI_PORT=<port>` to stdout for discovery
- Stderr is monitored asynchronously and logged with `[Python]` prefix
- UUID v4 used for shared secret
- Graceful shutdown with SIGTERM (Unix) / TerminateProcess (Windows)

**Current Blockers:** None

### ⏳ Phase 2: Pydantic AI Integration
**Status:** Not Started

**Tasks:**
- [ ] Set up Pydantic AI agent
- [ ] Configure provider support (Anthropic, OpenAI, Gemini, Ollama, OpenRouter)
- [ ] Implement conversation management
- [ ] Implement streaming responses
- [ ] Add error handling and recovery
- [ ] Test with each provider

**Dependencies:** Phase 1 complete

### ⏳ Phase 3: Tool Migration - Python Side
**Status:** Not Started

**Tasks:**
- [ ] Create `python/terminai_agent/tools/` module
- [ ] Implement `read_file` tool
- [ ] Implement `grep_files` tool
- [ ] Register tools with Pydantic AI agent
- [ ] Write unit tests (pytest)
- [ ] Test tool execution through agent

**Dependencies:** Phase 2 complete

### ⏳ Phase 4: Tool Migration - Rust Side
**Status:** Not Started

**Tasks:**
- [ ] Check out ag-ui-client SDK to `tmp-deps/`
- [ ] Create `src/llm/ag_ui_tools/` module
- [ ] Implement `suggest_command` as AG-UI tool
- [ ] Implement `read_scrollback` as AG-UI tool
- [ ] Register Rust tools with Python via AG-UI
- [ ] Test bidirectional tool calling
- [ ] Write integration tests

**Dependencies:** Phase 3 complete

### ⏳ Phase 5: AI Chat Process Refactoring
**Status:** Not Started

**Tasks:**
- [ ] Create new `AgUiClient` wrapper
- [ ] Replace `LLMClient` with `AgUiClient` in `AIChatProcess`
- [ ] Update streaming response handling
- [ ] Update conversation management
- [ ] Preserve command approval workflow
- [ ] Update error handling
- [ ] Test all UI interactions

**Dependencies:** Phase 4 complete

### ⏳ Phase 6: Cleanup and Testing
**Status:** Not Started

**Tasks:**
- [ ] Remove `src/llm/client.rs`
- [ ] Remove `src/llm/tools/` (old tools)
- [ ] Remove `src/llm/providers.rs`
- [ ] Remove `src/llm/prompts.rs` (move to Python)
- [ ] Update `src/llm/mod.rs`
- [ ] Remove rig-core from Cargo.toml
- [ ] Run full test suite
- [ ] Fix any broken tests
- [ ] Update documentation
- [ ] Final lint and build check

**Dependencies:** Phase 5 complete

## Key Decisions Log

### 2025-12-27: Initial Architecture Decisions

**Decision:** Use Python subprocess with AG-UI protocol
- **Rationale:** Better LLM ecosystem in Python, cleaner separation of concerns
- **Alternative Considered:** Keep everything in Rust with rig-core
- **Trade-offs:** Added IPC complexity, but better maintainability

**Decision:** Split tools between Rust and Python
- **Rationale:** Terminal operations need direct access to VT100 state (Rust), file operations are easier in Python
- **Tool Distribution:**
  - Rust: `suggest_command`, `read_scrollback`, future terminal tools
  - Python: `read_file`, `grep_files`, future analysis tools

**Decision:** Long-lived subprocess (not per-query)
- **Rationale:** Avoid Python startup overhead, maintain conversation state
- **Trade-offs:** Need proper lifecycle management, restart on crash

**Decision:** Auto-select port from range 18080-18099
- **Rationale:** Avoid port conflicts, allow multiple instances
- **Fallback:** OS-assigned port if range is exhausted

## Technical Notes

### Subprocess Communication Protocol

**Startup:**
```
Rust generates UUID → spawns `uv run python -m terminai_agent --secret <UUID>`
Python selects port → prints `AG_UI_PORT=<port>\n` to stdout
Rust reads stdout → connects to http://localhost:<port>
```

**Authentication:**
```
All HTTP requests include: X-AG-UI-Secret: <UUID>
Python middleware verifies secret or returns 401
```

**Logging:**
```
Python stderr → Rust async task → Rust log with [Python] prefix
```

### File Structure

**Python:**
```
python/
├── terminai_agent/
│   ├── __init__.py
│   ├── __main__.py          # Entry point
│   ├── server.py            # FastAPI server
│   ├── agent.py             # Pydantic AI agent
│   ├── config.py            # Provider configuration
│   └── tools/
│       ├── __init__.py
│       ├── read_file.py
│       └── grep_files.py
├── tests/
│   ├── test_tools.py
│   └── test_agent.py
└── pyproject.toml
```

**Rust:**
```
src/
├── llm/
│   ├── mod.rs              # Re-exports
│   ├── subprocess.rs       # Python subprocess management
│   └── ag_ui_tools/        # Rust tools exposed via AG-UI
│       ├── mod.rs
│       ├── suggest_command.rs
│       └── read_scrollback.rs
└── ai_proc/
    └── chat_process.rs     # Updated to use AgUiClient
```

## Current Issues

None yet.

## Testing Checklist

### Unit Tests
- [ ] Python tools (pytest)
- [ ] Rust AG-UI tools
- [ ] Subprocess management
- [ ] Port discovery
- [ ] Secret verification

### Integration Tests
- [ ] Full subprocess lifecycle
- [ ] Tool calling (Python → Rust)
- [ ] Tool calling (Rust → Python)
- [ ] Streaming responses
- [ ] Error recovery
- [ ] Provider switching

### Manual Tests
- [ ] Anthropic provider
- [ ] OpenAI provider
- [ ] Gemini provider
- [ ] Ollama provider
- [ ] OpenRouter provider
- [ ] Command suggestion and approval
- [ ] File reading
- [ ] File searching (grep)
- [ ] Scrollback reading
- [ ] Error scenarios (Python crash, API errors)

## Next Steps

1. Begin Phase 1: Python Subprocess Foundation
2. Create Python project structure with uv
3. Implement basic FastAPI server
4. Implement subprocess launch in Rust

---

**Progress Updates:** Will be added as implementation proceeds
