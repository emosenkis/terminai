# Termin.AI Development Progress

**Last Updated:** 2025-11-24 (Revised Strategy)
**Version:** 0.1.0-dev
**Status:** Clean binary approach - starting fresh

---

## Overview

This document tracks the development of Termin.AI following our **revised clean binary strategy**. We are building a minimal terminal wrapper that uses only the necessary mprocs PTY/VT100 code without any of its UI chrome.

---

## Strategic Pivot (2025-11-24)

### Previous Approach (Deprecated)
- ❌ Tried to integrate AI into existing mprocs multi-process UI
- ❌ Resulted in process list panes, help bars, window borders
- ❌ Not aligned with PRD: should be transparent single-shell wrapper

### New Approach (Current)
- ✅ **Separate clean binary** (`terminai`) that only imports what we need
- ✅ **No mprocs UI chrome** - no config parsing, no process list, no help bar
- ✅ **Single shell only** - launches user's $SHELL directly
- ✅ **AI overlay on demand** - Ctrl-Space shows overlay (even without API key configured)
- ✅ **Minimal dependencies** - only PTY handling and VT100 emulation from mprocs

---

## Current Status: Phase 0 - Clean Binary Foundation

### What's Complete

#### ✅ Binary Structure
- Created `src/bin/terminai.rs` - clean implementation
- Created `src/lib.rs` - exports modules for binary use
- Updated `Cargo.toml` with lib and second binary target
- **Build Status:** ✅ Compiles successfully

#### ✅ Core AI Modules (From Previous Work)
All AI functionality is implemented and ready to integrate:
- **LLM Client** (`src/llm/`) - Multi-provider support (Anthropic, OpenAI, Gemini, Ollama)
- **Command Parser** (`src/command/parser.rs`) - Extracts commands from markdown
- **Safety Validator** (`src/command/validator.rs`) - 3-tier risk classification
- **Command Executor** (`src/command/executor.rs`) - PTY command injection
- **Privacy Filter** (`src/privacy/`) - Redacts sensitive data
- **AI Chat Process** (`src/ai_proc/`) - Conversation management
- **Context Extractor** (`src/ai_proc/context.rs`) - Terminal history capture
- **Chat UI** (`src/ai_proc/ui.rs`) - Ratatui-based overlay rendering

**Test Coverage:** 34/34 unit tests passing

#### ✅ Current terminai.rs Skeleton
```rust
struct App {
  ai_process: Option<AIChatProcess>,
  ai_visible: bool,
  shell_command: String,
}
```

**What Works:**
- Terminal raw mode setup
- AI initialization (if ANTHROPIC_API_KEY set)
- Keyboard input handling (Ctrl-C, Ctrl-Space, ESC)
- Clean shutdown

**What's Missing:**
- Shell PTY spawning
- Terminal output capture & rendering
- Input passthrough to shell
- AI overlay rendering
- Context extraction
- Command execution

---

## Revised Implementation Plan

### Phase 0: Clean Binary Foundation ✅ COMPLETE (100%)

**Goal:** Minimal working binary that launches a shell with no UI chrome

**Tasks:**
- [x] Create separate binary target (`terminai`)
- [x] Setup library exports (`lib.rs`)
- [x] Basic terminal mode handling
- [x] Keyboard input skeleton
- [x] **DONE:** Spawn shell via PTY (caac964)
- [x] **DONE:** VT100 terminal emulation integration
- [x] **DONE:** Keyboard passthrough to shell
- [x] **DONE:** Render shell output to screen (ratatui)
- [x] **DONE:** Handle terminal resize events

**Deliverable:** ✅ Clean shell wrapper with zero UI elements

**Part 1 - PTY & Event Loop (2025-11-24):**
- ✅ Implemented Shell struct for PTY management (pattern from mprocs/inst.rs)
- ✅ VT100 parser integration for terminal emulation
- ✅ Keyboard encoding using mprocs' encode_key
- ✅ Event loop with tokio::select! for shell events + keyboard
- ✅ Ctrl-Space hotkey detection

**Part 2 - Terminal Rendering (2025-11-25):**
- ✅ Created TerminalWidget implementing ratatui Widget trait (pattern from mprocs/ui_term.rs)
- ✅ Added Terminal<CrosstermBackend> for screen rendering
- ✅ Implemented App::render() with cell-by-cell VT100 to TUI conversion
- ✅ Implemented Shell::resize() for PTY and VT100 resize handling
- ✅ Wired resize events to Shell::resize() in event loop
- ✅ Enabled crossterm feature for ratatui in Cargo.toml
- ✅ All 34 tests passing, builds successfully

---

### Phase 1: AI Overlay Integration ✅ COMPLETE (100%)

**Goal:** Show AI overlay on Ctrl-Space, even without API key

**Tasks:**
- [x] Render AI overlay when `ai_visible = true`
- [x] Show "API key not configured" message if no key
- [x] Handle overlay keyboard input (typing, ESC to close)
- [x] Proper overlay positioning (80% x 70%, centered)
- [x] Preserve shell output beneath overlay

**Deliverable:** ✅ Visible AI overlay that users can interact with

**Implementation (2025-11-25):**
- ✅ Added imports for AIChatUI and ratatui layout components
- ✅ Implemented centered_rect() helper function for overlay positioning
- ✅ Modified render() to show AI overlay when ai_visible = true
- ✅ Integrated AIChatUI widget from ai_proc/ui.rs
- ✅ Added "not configured" message when ANTHROPIC_API_KEY not set
- ✅ Implemented keyboard routing for AI overlay:
  - Regular characters append to input buffer
  - Backspace deletes last character
  - ESC closes overlay
  - Ctrl-Space toggles overlay
- ✅ All 34 tests passing, builds successfully

---

### Phase 2: LLM Integration ✅ COMPLETE (100%)

**Goal:** Send messages to LLM and display responses

**Tasks:**
- [x] Wire up Enter key to send messages
- [x] Extract terminal context (history, cwd, exit code)
- [x] Apply privacy filtering
- [x] Send to LLM (non-streaming for MVP)
- [x] Display responses in overlay
- [x] Handle LLM errors gracefully

**Deliverable:** ✅ Working AI chat with context awareness

**Implementation (2025-11-25):**
- ✅ Created extract_context() method to extract VT100 screen content
- ✅ Extract up to 500 lines of terminal history (as per PRD)
- ✅ Extract current working directory
- ✅ Privacy filtering applied via send_input_with_context
- ✅ Wire up Enter key to send messages to LLM
- ✅ Handle async LLM calls in event loop
- ✅ Display AI responses automatically in conversation history
- ✅ Error logging for failed LLM requests
- ✅ All 34 tests passing, builds successfully

**Note:** Streaming responses deferred to future enhancement (non-blocking for MVP)

---

### Phase 3: Command Execution (Week 4)

**Goal:** Parse commands from AI responses and execute with approval

**Tasks:**
- [ ] Detect commands in AI responses (markdown code blocks)
- [ ] Classify command safety (Safe/Caution/Dangerous)
- [ ] Show approval prompt for Caution/Dangerous commands
- [ ] Handle Y/N approval keys
- [ ] Inject approved commands into shell PTY as keyboard input
- [ ] Show execution feedback

**Deliverable:** End-to-end AI command suggestion and execution

---

### Phase 4: Polish & Testing (Week 5)

**Goal:** Production-ready release

**Tasks:**
- [ ] Cross-platform testing (Linux, macOS)
- [ ] Multiple shell testing (bash, zsh, fish)
- [ ] Error handling improvements
- [ ] Performance optimization
- [ ] Documentation (README, examples)
- [ ] Release preparation

---

## Technical Architecture

### Binary Targets

**`termin` (main.rs):**
- ✅ Reverted to upstream mprocs
- Not used for termin.ai
- Kept for reference/comparison

**`terminai` (bin/terminai.rs):**
- 🚧 New clean implementation
- Only imports necessary mprocs modules
- No UI chrome, config parsing, or multi-process logic

### Module Dependencies

**What We Use from mprocs:**
```
vt100/          → Terminal emulation (VT100 parsing, screen buffer)
proc/           → PTY management (spawn process, I/O)
term/           → Terminal abstractions
key.rs          → Key event types
event.rs        → Event types
```

**What We DON'T Use:**
```
app.rs          → mprocs application (multi-process manager)
config.rs       → Process config file parsing
ui_*.rs         → mprocs UI components (process list, help bar)
modal/          → mprocs modal dialogs
kernel/         → Multi-process kernel
settings.rs     → mprocs settings
```

**What We Built (Termin.AI):**
```
llm/            → Multi-provider LLM client
ai_proc/        → AI chat process & UI
command/        → Command parsing, validation, execution
privacy/        → Sensitive data filtering
```

---

## Code Statistics

### Current State (2025-11-24)

**Lines of Code:**
- Termin.AI modules: ~984 LOC
- mprocs base (inherited): ~8000 LOC
- New terminai binary: ~150 LOC
- **Total:** ~9100 LOC

**Files:**
- New files created: 14 (13 AI modules + 1 binary)
- Modified from mprocs: 2 (main.rs reverted, lib.rs new)

**Build Status:**
```
$ cargo build --bin terminai
   Compiling termin v0.1.0
    Finished `dev` profile
```
✅ **All builds successful**

**Test Status:**
```
$ cargo test
running 34 tests
34 tests passed
```
✅ **All tests passing**

---

## Next Immediate Steps

### ✅ Phase 0, 1, 2 Complete - Moving to Phase 3

**Phase 0 Status:** All tasks complete, clean shell wrapper working
**Phase 1 Status:** All tasks complete, AI overlay rendering and input working
**Phase 2 Status:** All tasks complete, LLM integration with context extraction working

### Priority 1: Command Detection & Parsing (Phase 3 - Next 2-3 hours)

**Goal:** Detect and parse commands from AI responses

**Implementation:**
1. Use CommandParser to extract commands from markdown code blocks
2. Classify command safety using SafetyValidator
3. Display pending command in approval UI
4. Handle approval/rejection workflow

**Reference Code:**
- `src/command/parser.rs` - CommandParser already implemented
- `src/command/validator.rs` - SafetyValidator with 3-tier risk levels
- `src/ai_proc/ui.rs` - render_approval_prompt() already implemented

### Priority 2: Command Execution (Phase 3 - Next 2-3 hours)

**Goal:** Execute approved commands by injecting into shell

**Implementation:**
1. On approval, inject command into shell PTY as keyboard input
2. Use encode_key to convert command string to PTY input
3. Add newline to execute command
4. Log command execution for debugging
5. Handle rejection (just clear pending command)

**Reference Code:**
- `src/command/executor.rs` - CommandExecutor (needs adaptation for single shell)
- Shell::send_key() - already working for keyboard injection

---

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| PTY integration complex | High | Study mprocs' proc module, it's already working |
| Rendering without mprocs' app.rs | Medium | Use ratatui directly, simpler than mprocs |
| Context extraction from VT100 | Medium | Already implemented in `ai_proc/context.rs` |
| Command injection unsafe | High | Already have SafetyValidator with tests |

---

## Success Criteria (MVP v0.1.0)

From ORIGINAL_PRD.md:

| Requirement | Status | Notes |
|-------------|--------|-------|
| Transparent shell wrapper | 🚧 In Progress | Binary structure ready |
| No UI chrome (borders, help) | ✅ Achieved | Clean binary approach |
| Single shell only | ✅ Planned | No multi-process logic |
| Ctrl-Space activates AI | ✅ Skeleton Ready | Just needs rendering |
| AI overlay visible without API key | 🚧 Todo | Will show config message |
| Command approval system | ✅ Ready | SafetyValidator complete |
| Context-aware AI | ✅ Ready | ContextExtractor complete |
| Multi-provider support | ✅ Complete | Anthropic, OpenAI, Gemini, Ollama |
| Works with bash/zsh/fish | 🚧 Todo | Will use $SHELL |

---

## Lessons Learned

### What Went Wrong

1. **Initial Integration Approach:**
   - Tried to integrate AI into mprocs' existing app structure
   - Resulted in inheriting unwanted UI components
   - Process list panes and help bars violated PRD

2. **Lack of Clear Separation:**
   - Didn't realize extent of mprocs' UI until runtime
   - Should have started with clean binary from day 1

### What Went Right

1. **AI Modules Well-Architected:**
   - Clean separation of concerns
   - Comprehensive test coverage
   - Ready to drop into new binary

2. **mprocs as Library:**
   - PTY and VT100 code is exactly what we need
   - No need to rewrite terminal virtualization
   - Can use as library without UI baggage

### Going Forward

1. **Start Clean:** Always prefer minimal new binary over extending existing
2. **Test Early:** Should have run binary early to catch UI issues
3. **Clear Boundaries:** Distinguish "code library" from "application structure"

---

## Timeline

### Completed (Weeks 1-3)
- ✅ Core AI module implementation
- ✅ LLM client, parsing, validation
- ✅ Context extraction, privacy filtering
- ✅ UI components, chat process

### In Progress (Week 4 - Current)
- 🚧 Clean binary foundation
- 🚧 Shell PTY integration
- 🚧 AI overlay rendering

### Remaining (Week 5)
- Command execution workflow
- Testing and polish
- Documentation
- Release preparation

**Revised Completion:** End of Week 5 (on track)

---

## References

- **ORIGINAL_PRD.md** - Product requirements (source of truth)
- **IMPLEMENTATION_PLAN.md** - Technical architecture (needs update)
- **CLAUDE.md** - Development guidelines
- **MPROCS_BORROWED.md** - What we use from mprocs

---

**Last Commit:** Creating clean binary foundation
**Next Milestone:** Shell rendering in terminai binary
