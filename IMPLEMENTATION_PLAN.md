# Termin.AI Implementation Plan
## Leveraging mprocs Technology for Single-Shell AI Terminal

**Version:** 3.0 (Revised - Product Clarity)
**Date:** 2025-11-14
**Base Technology:** mprocs v0.7.3 (code reuse, not extension)
**Strategy:** Single shell with AI overlay using mprocs' virtualization technology

---

## Executive Summary

**Product Vision:** A single terminal window that runs the user's default shell with an AI assistant that can overlay on top.

**Technical Strategy:** Use mprocs as a **code library** - borrowing its excellent PTY handling, terminal virtualization, and TUI rendering - but **not as a product to extend**.

### What We're Building
- ✅ Single terminal window (not multi-process tabs)
- ✅ Runs user's default shell by default
- ✅ AI can pop up as overlay over the shell
- ✅ AI can send commands to the shell
- ✅ Terminal I/O is virtualized (enabling context capture)

### What We're NOT Building
- ❌ Multi-process manager with config files
- ❌ Tab-based interface for multiple processes
- ❌ Extension of mprocs as a product
- ❌ Config-driven process launcher

### Key Technologies from mprocs
- ✅ PTY handling (via portable-pty)
- ✅ TUI interface (via ratatui)
- ✅ Terminal I/O capture and virtualization
- ✅ VT100 emulation
- ✅ Keyboard input routing

**Upstream Strategy:** Cherry-pick improvements from mprocs' virtualization code, but maintain our own distinct product architecture.

---

## Table of Contents

1. [What mprocs Already Provides](#what-mprocs-already-provides)
2. [What We Need to Add](#what-we-need-to-add)
3. [Architecture: Extension Strategy](#architecture-extension-strategy)
4. [Module Additions](#module-additions)
5. [Integration Points](#integration-points)
6. [Development Phases](#development-phases)
7. [Upstream Compatibility](#upstream-compatibility)

---

## What mprocs Already Provides

### 1. **PTY Management** (`src/proc/`)
- Process spawning and management
- PTY creation and I/O handling
- Process lifecycle (start, stop, restart)
- Signal handling (SIGTERM, SIGKILL)
- Auto-restart capabilities

**Files:**
- `src/proc/proc.rs` - Process implementation
- `src/proc/inst.rs` - Process instance
- `src/proc/msg.rs` - Process messages
- `src/proc/view.rs` - Process view

### 2. **Terminal Emulation** (`src/vt100/`, `src/term/`)
- VT100 terminal emulation
- Terminal screen buffer
- Scrollback support (configurable size)
- Cell-based rendering
- ANSI escape sequence parsing

**Files:**
- `src/vt100/term.rs` - Terminal implementation
- `src/vt100/screen.rs` - Screen buffer
- `src/vt100/cell.rs` - Cell data structure

### 3. **TUI Framework** (`src/ui_*.rs`, `src/widgets/`)
- Process list panel
- Terminal output panel
- Keymap help panel
- Modal dialogs (add/remove/rename process)
- Copy mode with selection
- Zoom functionality

**Files:**
- `src/ui_procs.rs` - Process list UI
- `src/ui_term.rs` - Terminal output UI
- `src/ui_keymap.rs` - Keymap display
- `src/ui_zoom_tip.rs` - Zoom tip
- `src/widgets/` - Reusable UI widgets

### 4. **Input Handling** (`src/key.rs`, `src/keymap.rs`, `src/event.rs`)
- Keyboard event processing
- Configurable key bindings
- Three keymap contexts: process list, terminal, copy mode
- Mouse support (scrolling)

### 5. **Configuration System** (`src/config.rs`, `src/settings.rs`)
- YAML/JSON/Lua config loading
- Global config (`~/.config/mprocs/mprocs.yaml`)
- Local config (`./mprocs.yaml`)
- Settings merging
- Schema validation

### 6. **Application Architecture** (`src/app.rs`, `src/kernel/`)
- Message-passing kernel
- Process communication
- Client-server architecture
- Remote control via TCP

**Files:**
- `src/app.rs` - Main application logic
- `src/kernel/kernel.rs` - Process kernel
- `src/kernel/proc.rs` - Kernel process abstraction
- `src/server/` - TCP server for remote control
- `src/client.rs` - Client implementation

---

## Architecture: Single Shell with AI Overlay

### Design Philosophy

**Goal:** Build a transparent shell wrapper that feels like a normal terminal until you invoke AI assistance.

**Core Principle:** Termin.AI is NOT an extension of mprocs. We're building a **new product** that borrows mprocs' **technical implementations** for PTY and terminal virtualization.

**Key Differences from mprocs:**
1. **Single Shell Focus:** Launch user's default shell, not multiple configured processes
2. **No Process Config:** No YAML/config files for process management
3. **Transparent by Default:** User sees their shell, not a process manager UI
4. **AI Overlay:** AI appears on demand, overlaying the terminal
5. **Command Injection:** AI can send input to the shell, not manage separate processes

### Upstream Relationship

**Think of it as:** Using mprocs as a **library** of terminal virtualization code, not as a **base product** to extend.

**Merge Strategy:**
- Track mprocs improvements in PTY/VT100/rendering code
- Selectively merge relevant bug fixes and improvements
- Maintain our own distinct application architecture
- No need to preserve mprocs' multi-process semantics

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Termin.AI Application                     │
│                (Single Shell with AI Overlay)                 │
│                                                               │
│  ┌────────────────────────────────────────────────────────┐ │
│  │                  Application Core                        │ │
│  │                                                          │ │
│  │  ┌──────────────────────────────────────┐              │ │
│  │  │      Shell Virtualization Layer      │              │ │
│  │  │  (Borrowed from mprocs)              │              │ │
│  │  │                                       │              │ │
│  │  │  ┌─────────┐  ┌─────────┐           │              │ │
│  │  │  │   PTY   │  │  VT100  │           │              │ │
│  │  │  │ Manager │  │ Emulator│           │              │ │
│  │  │  └─────────┘  └─────────┘           │              │ │
│  │  │                                       │              │ │
│  │  │  ┌─────────┐  ┌─────────┐           │              │ │
│  │  │  │ Terminal│  │  I/O    │           │              │ │
│  │  │  │  Buffer │  │ Capture │           │              │ │
│  │  │  └─────────┘  └─────────┘           │              │ │
│  │  └──────────────────────────────────────┘              │ │
│  │                      │                                   │ │
│  │                      ▼                                   │ │
│  │  ┌──────────────────────────────────────┐              │ │
│  │  │        User Shell Process            │              │ │
│  │  │   (bash/zsh/fish - user's default)   │              │ │
│  │  └──────────────────────────────────────┘              │ │
│  │                                                          │ │
│  └────────────────────────────────────────────────────────┘ │
│                             │                                 │
│                             ▼                                 │
│  ┌────────────────────────────────────────────────────────┐ │
│  │               UI Layer (ratatui)                        │ │
│  │                                                          │ │
│  │  Normal Mode:                                           │ │
│  │  ┌──────────────────────────────────────────────────┐  │ │
│  │  │                                                   │  │ │
│  │  │         [Terminal Output - Full Screen]          │  │ │
│  │  │                                                   │  │ │
│  │  │         $ your command here_                     │  │ │
│  │  │                                                   │  │ │
│  │  └──────────────────────────────────────────────────┘  │ │
│  │                                                          │ │
│  │  AI Mode (Ctrl-Space):                                  │ │
│  │  ┌──────────────────────────────────────────────────┐  │ │
│  │  │  [Terminal Output - Background]                  │  │ │
│  │  │  ┌────────────────────────────────────────────┐  │  │ │
│  │  │  │  AI Chat Overlay                           │  │  │ │
│  │  │  │                                             │  │  │ │
│  │  │  │  You: why did this fail?                   │  │  │ │
│  │  │  │  AI: The error indicates...                │  │  │ │
│  │  │  │      Try: sudo apt install foo             │  │  │ │
│  │  │  │      [Execute] [Edit] [Cancel]             │  │  │ │
│  │  │  │                                             │  │  │ │
│  │  │  │  Your message: _                           │  │  │ │
│  │  │  └────────────────────────────────────────────┘  │  │ │
│  │  └──────────────────────────────────────────────────┘  │ │
│  │                                                          │ │
│  └────────────────────────────────────────────────────────┘ │
│                             │                                 │
│                             ▼                                 │
│  ┌────────────────────────────────────────────────────────┐ │
│  │              AI Assistant Module                        │ │
│  │                                                          │ │
│  │  ┌──────────┐  ┌──────────┐  ┌────────────┐           │ │
│  │  │   LLM    │  │  Context │  │  Command   │           │ │
│  │  │  Client  │  │ Extractor│  │  Parser    │           │ │
│  │  └──────────┘  └──────────┘  └────────────┘           │ │
│  │                                                          │ │
│  │  ┌──────────┐  ┌──────────┐  ┌────────────┐           │ │
│  │  │  Safety  │  │ Privacy  │  │  Command   │           │ │
│  │  │ Validator│  │  Filter  │  │  Injector  │           │ │
│  │  └──────────┘  └──────────┘  └────────────┘           │ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

### User Experience Flow

**1. Launch:**
```bash
$ terminai
# Launches your default shell ($SHELL)
# Looks and feels like normal terminal
# All I/O is virtualized for context capture
```

**2. Normal Usage:**
```
- Terminal shows your shell in full screen
- All commands work normally
- No visible difference from regular terminal
- Terminal history is being captured
```

**3. AI Invocation (Ctrl-Space):**
```
- AI overlay appears over terminal
- Terminal content remains visible beneath
- Can type questions/requests to AI
- AI has full context of terminal history
```

**4. Command Execution:**
```
- AI suggests commands
- User approves/edits/cancels
- Commands are injected into shell
- Output appears in terminal
- AI overlay can stay open or close
```

---

## Module Additions

### New Module 1: `src/llm/` - LLM Client

**Purpose:** Multi-provider LLM API client

**Files to Create:**
- `src/llm/mod.rs` - Module exports
- `src/llm/client.rs` - Unified LLM client (using genai crate)
- `src/llm/providers.rs` - Provider configurations
- `src/llm/streaming.rs` - Stream handling
- `src/llm/prompts.rs` - System prompt templates

**Dependencies:**
```toml
[dependencies]
genai = "0.3"
reqwest = { version = "0.11", features = ["json", "stream", "rustls-tls"] }
```

**Key Types:**
```rust
pub struct LLMClient {
    client: genai::Client,
    provider: Provider,
    model: String,
}

pub enum Provider {
    Anthropic,
    OpenAI,
    Gemini,
    Ollama,
}

pub struct ChatRequest {
    message: String,
    context: TerminalContext,
    history: Vec<Message>,
}

pub struct TerminalContext {
    history_lines: Vec<String>,
    cwd: PathBuf,
    last_exit_code: Option<i32>,
}
```

**Integration:** Standalone module, no changes to mprocs core required.

---

### New Module 2: `src/ai_proc/` - AI Assistant Process

**Purpose:** Special process type for LLM chat interface

**Files to Create:**
- `src/ai_proc/mod.rs` - Module exports
- `src/ai_proc/chat_process.rs` - AI chat process implementation
- `src/ai_proc/ui.rs` - Chat UI rendering
- `src/ai_proc/context.rs` - Terminal context extraction

**Key Types:**
```rust
pub struct AIChatProcess {
    llm_client: LLMClient,
    conversation: Vec<Message>,
    input_buffer: String,
    awaiting_approval: Option<Command>,
}

impl AIChatProcess {
    pub fn new(config: AIConfig) -> Self;
    pub async fn send_message(&mut self, msg: String);
    pub fn extract_context(&self, procs: &[ProcView]) -> TerminalContext;
    pub fn render(&self, frame: &mut Frame, area: Rect);
}
```

**Integration:** Register as a new process type in the kernel. Minimal changes to `src/app.rs` to add AI process.

---

### New Module 3: `src/command/` - Command Parser & Validator

**Purpose:** Extract and validate commands from LLM responses

**Files to Create:**
- `src/command/mod.rs` - Module exports
- `src/command/parser.rs` - Parse bash code blocks from markdown
- `src/command/validator.rs` - Safety checking
- `src/command/executor.rs` - Command execution in target process

**Key Types:**
```rust
pub struct CommandParser;

impl CommandParser {
    pub fn extract_commands(markdown: &str) -> Vec<String>;
}

pub struct SafetyValidator {
    safe_commands: HashSet<String>,
    dangerous_commands: HashSet<String>,
}

pub enum RiskLevel {
    Safe,
    Caution,
    Dangerous,
}

impl SafetyValidator {
    pub fn assess_risk(&self, cmd: &str) -> RiskLevel;
}
```

**Integration:** Used by AI process, no changes to mprocs core.

---

### New Module 4: `src/privacy/` - Privacy Filter

**Purpose:** Redact sensitive information from terminal context

**Files to Create:**
- `src/privacy/mod.rs` - Module exports
- `src/privacy/filter.rs` - Pattern-based filtering

**Key Types:**
```rust
pub struct PrivacyFilter {
    patterns: Vec<Regex>,
}

impl PrivacyFilter {
    pub fn new() -> Self;
    pub fn filter(&self, text: &str) -> String;
}

const DEFAULT_PATTERNS: &[&str] = &[
    r"password[=:]\s*\S+",
    r"api[_-]?key[=:]\s*\S+",
    r"AWS_[A-Z_]+_KEY[=:]\s*\S+",
];
```

**Integration:** Used by context extractor, standalone module.

---

## Integration Strategy with mprocs Code

### What We're Reusing vs Replacing

**Reusing (as library code):**
- `src/vt100/` - Terminal emulation (VT100, screen buffer, cells)
- `src/term/` - Terminal abstractions
- `src/proc/` - PTY management (we'll use for single shell)
- `src/key.rs`, `src/event.rs` - Keyboard/input handling
- Portions of UI code from `src/widgets/`

**Replacing (different product architecture):**
- `src/main.rs` - New main for single-shell app
- `src/app.rs` - New app structure (not multi-process manager)
- `src/config.rs` - Simplified config (no process configs)
- `src/ui_*.rs` - New UI (single terminal + AI overlay, not process list + tabs)
- `src/kernel/` - Simplified (single shell, not multiple processes)
- `src/server/`, `src/client.rs` - Not needed initially

### New Application Structure

**New `src/main.rs`:**
```rust
// terminai/src/main.rs
use terminai::{App, Config};

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Load minimal config (AI settings, keybindings)
    let config = Config::load()?;

    // 2. Detect user's shell ($SHELL)
    let shell = std::env::var("SHELL")
        .unwrap_or_else(|_| "/bin/bash".to_string());

    // 3. Create and run app
    let mut app = App::new(config, shell)?;
    app.run().await?;

    Ok(())
}
```

**New `src/app.rs`:**
```rust
pub struct App {
    // Single shell process
    shell_proc: ShellProcess,

    // Virtual terminal for shell
    terminal: Term,  // from mprocs vt100

    // AI assistant (overlay)
    ai: Option<AIAssistant>,
    ai_visible: bool,

    // UI state
    mode: AppMode,  // Normal, AI, Copy

    // Configuration
    config: Config,
}

pub enum AppMode {
    Normal,      // Pass-through to shell
    AI,          // AI overlay visible
    Copy,        // Copy mode (from mprocs)
}

impl App {
    pub async fn run(&mut self) -> Result<()> {
        loop {
            // Handle input
            match self.mode {
                AppMode::Normal => self.handle_normal_mode().await?,
                AppMode::AI => self.handle_ai_mode().await?,
                AppMode::Copy => self.handle_copy_mode().await?,
            }

            // Render
            self.render()?;
        }
    }

    fn handle_normal_mode(&mut self) -> Result<()> {
        // Check for Ctrl-Space -> switch to AI mode
        // Otherwise pass input directly to shell
    }

    fn handle_ai_mode(&mut self) -> Result<()> {
        // AI chat interface active
        // Can send messages, approve commands
        // ESC to return to normal mode
    }
}
```

**Simplified `src/config.rs`:**
```rust
#[derive(Deserialize)]
pub struct Config {
    // AI settings
    pub ai: AIConfig,

    // Keybindings
    pub keybindings: KeyBindings,

    // Terminal settings
    pub terminal: TerminalConfig,
}

#[derive(Deserialize)]
pub struct AIConfig {
    pub provider: String,
    pub model: String,
    pub api_key_env: String,

    // Safety settings
    pub safe_commands: Vec<String>,
    pub dangerous_commands: Vec<String>,
}

// NO process management config!
// NO procs: section!
```

### Changes Needed to mprocs Code

**Minimal modifications to reused code:**
- Most mprocs code in `src/vt100/`, `src/proc/`, `src/term/` can be used as-is
- May need to adjust imports/module structure
- Remove dependencies on mprocs' multi-process kernel
- Document with `// TERMINAI:` markers where we modify

---

## Development Phases

### Phase 1: Setup & Foundation (Week 1)

**Goal:** Verify mprocs builds and runs, set up project structure

**Tasks:**
- [x] Clone mprocs repository
- [x] Verify cargo build works
- [x] Run mprocs and test basic functionality
- [x] Create new module directories
- [x] Add LLM dependencies to Cargo.toml
- [x] Set up testing framework

**Deliverable:** ✅ COMPLETED - mprocs builds and runs with new dependencies added

**Status:** All module directories created (llm/, ai_proc/, command/, privacy/). Dependencies added (genai 0.3, regex 1.10).

---

### Phase 2: LLM Client Module (Week 2)

**Goal:** Standalone LLM client that works independently

**Tasks:**
- [x] Implement `src/llm/client.rs` using genai crate
- [x] Add support for Anthropic Claude
- [x] Add support for OpenAI GPT-4
- [x] Implement streaming responses (using exec_chat_stream)
- [x] Write unit tests
- [x] Provider abstraction (Anthropic, OpenAI, Gemini, Ollama)

**Deliverable:** ✅ COMPLETED - LLM client can chat with AI providers

**Status:** Full implementation complete including:
- Non-streaming chat via `send_message()`
- Streaming chat via `send_message_stream()` with proper ChatStreamEvent handling
- Multi-provider support through genai crate
- System prompts and context formatting
- Unit tests passing (test_terminal_context_creation, test_empty_context, etc.)

**Testing:**
```bash
# Test LLM client independently
cargo test --package termin --lib llm::tests
# Result: All tests passing ✅
```

---

### Phase 3: Command Parser & Safety (Week 3)

**Goal:** Parse and validate commands from LLM responses

**Tasks:**
- [x] Implement markdown code block parser
- [x] Implement SafetyValidator with risk levels
- [x] Add privacy filter
- [x] Implement command executor for PTY injection
- [x] Write comprehensive tests

**Deliverable:** ✅ COMPLETED - Can extract and classify commands safely

**Status:** Full implementation complete including:
- CommandParser extracts bash/sh/shell code blocks from markdown
- SafetyValidator with 3 risk levels (Safe, Caution, Dangerous)
- PrivacyFilter with comprehensive regex patterns (API keys, passwords, AWS credentials, JWT, emails, etc.)
- CommandExecutor sends commands to PTY via Key events
- Context extraction from terminal buffer (ProcView integration)
- All unit tests passing (34 tests total)

---

### Phase 4: AI Chat Process (Week 4)

**Goal:** AI process that renders in mprocs UI

**Tasks:**
- [x] Create AIChatProcess struct
- [x] Implement chat UI rendering (AIChatUI with conversation/input/approval views)
- [x] Add input handling (buffer management, send/clear)
- [x] Implement context extraction from terminal (ProcView integration)
- [ ] **IN PROGRESS:** Test as standalone process in mprocs

**Deliverable:** 🚧 IN PROGRESS - AI chat infrastructure ready, needs app integration

**Status:** Core functionality complete:
- AIChatProcess manages conversation state and LLM interaction
- UI rendering implemented with ratatui (conversation history, input box, approval popup)
- Context extraction working (reads terminal cells from Screen API)
- Privacy filtering applied to context before sending to LLM
- Command parsing and safety validation integrated
- Needs: Integration into app.rs event loop

---

### Phase 5: Integration (Week 5)

**Goal:** Connect AI process to mprocs kernel

**Tasks:**
- [ ] Add AI config to mprocs config system (AIConfig defined but not loaded)
- [ ] Add activation key binding (Ctrl-Space defined in PRD)
- [ ] Integrate AI process with process manager (main blocker)
- [ ] Wire up command approval and execution flow
- [ ] Test full workflow

**Deliverable:** ⏳ NOT STARTED - Can activate AI chat with key press in running app

**Blockers:** Requires understanding of mprocs kernel architecture and event loop

---

### Phase 6: Command Execution (Week 6)

**Goal:** Execute approved commands in target processes

**Tasks:**
- [x] Implement command executor (CommandExecutor with PTY key injection)
- [ ] Add process selection for command target
- [ ] Implement approval workflow UI interaction
- [ ] Handle command output streaming (already captured by terminal buffer)
- [x] Add error handling

**Deliverable:** 🚧 PARTIALLY COMPLETE - Executor ready, needs approval workflow integration

---

### Phase 7: Polish & Testing (Week 7)

**Goal:** Production-ready, well-tested

**Tasks:**
- [ ] Cross-platform testing (Linux, macOS)
- [ ] Performance optimization
- [ ] Error handling improvements
- [ ] Documentation
- [ ] User testing

**Deliverable:** Stable, tested application

---

### Phase 8: Documentation & Release (Week 8)

**Goal:** Public release

**Tasks:**
- [ ] Write user documentation
- [ ] Create example configurations
- [ ] Record demo video
- [ ] Prepare release binaries
- [ ] Publish to GitHub

**Deliverable:** v0.1.0 release

---

## Upstream Relationship with mprocs

### Cherry-Picking Strategy

**Key Principle:** We're NOT maintaining fork compatibility. We're using mprocs code as a **starting point** for terminal virtualization.

**What This Means:**
1. **Initial Integration:** Copy mprocs v0.7.3 code we need (vt100, proc, term)
2. **Heavy Modification:** Rewrite main app architecture for single-shell usage
3. **Selective Updates:** Monitor mprocs for improvements to terminal handling
4. **Manual Cherry-Picks:** Pull in specific bug fixes or enhancements as needed

### Tracking Borrowed Code

**Document in `MPROCS_BORROWED.md`:**
```markdown
# Code Borrowed from mprocs v0.7.3

## Modules Used As-Is (or Nearly)

### src/vt100/
**Source:** mprocs v0.7.3
**Files:** term.rs, screen.rs, cell.rs, grid.rs, parser.rs
**Purpose:** VT100 terminal emulation
**Modifications:** Minimal (imports only)
**Update Strategy:** Monitor mprocs for bug fixes in VT100 parsing

### src/proc/
**Source:** mprocs v0.7.3
**Files:** proc.rs, inst.rs (simplified)
**Purpose:** PTY management
**Modifications:** Removed multi-process kernel dependencies
**Update Strategy:** Cherry-pick PTY handling improvements

### src/term/
**Source:** mprocs v0.7.3
**Files:** term.rs
**Purpose:** Terminal abstractions
**Modifications:** Minor (simplified)
**Update Strategy:** Monitor for terminal rendering improvements

## Modules Heavily Rewritten

### src/app.rs
**Based On:** mprocs app.rs
**Status:** Completely rewritten for single-shell architecture
**Similarity:** ~10% (basic structure only)

### src/config.rs
**Based On:** mprocs config.rs
**Status:** Replaced (different config schema)
**Similarity:** 0% (entirely different purpose)

### src/ui_*.rs
**Based On:** mprocs UI modules
**Status:** Partially borrowed (widgets), mostly new (layouts)
**Similarity:** ~30% (widget code reused)

## Update Monitoring

### High Priority (Monitor Closely)
- VT100 parser fixes
- PTY handling improvements
- Terminal rendering optimizations
- Unicode/emoji handling

### Medium Priority (Review Occasionally)
- Input handling improvements
- Widget enhancements
- Performance optimizations

### Low Priority (Informational Only)
- Multi-process features
- Remote control features
- mprocs-specific config changes
```

### Contributing Back to mprocs

**If we discover:**
- Bug fixes in VT100 parsing
- Improvements to PTY handling
- Better terminal rendering techniques
- Unicode edge cases

**We should:**
- Submit PR to mprocs
- Credit both projects
- Help upstream community

### Long-Term Strategy

**Years 1-2:**
- Monitor mprocs releases
- Selectively cherry-pick improvements
- Document all borrowed code updates

**Year 3+:**
- Consider if terminal virtualization code should be split into separate library
- Both Termin.AI and mprocs could depend on shared terminal lib
- Reduce duplication across projects

### Not a Fork

**Important:** Termin.AI is **not a fork** of mprocs:
- Different product vision
- Different user experience
- Different architecture
- Happens to share terminal virtualization code

**Analogy:** Like how many projects use `tokio` for async runtime, we're using mprocs' terminal virtualization as a foundation library.

---

## Configuration

### Termin.AI Configuration (~/.config/terminai/config.toml)

**Simple, focused configuration - no process management:**

```toml
[general]
# Shell to launch (default: $SHELL environment variable)
# shell = "/bin/bash"  # optional override
log_level = "info"

[ai]
provider = "anthropic"
model = "claude-3-5-sonnet-20241022"
api_key_env = "ANTHROPIC_API_KEY"

[ai.providers.anthropic]
api_key_env = "ANTHROPIC_API_KEY"
models = ["claude-3-5-sonnet-20241022", "claude-3-opus-20240229"]

[ai.providers.openai]
api_key_env = "OPENAI_API_KEY"
models = ["gpt-4-turbo", "gpt-3.5-turbo"]

[ai.providers.ollama]
endpoint = "http://localhost:11434"
models = ["llama3.2", "codellama"]

[safety]
safe_commands = ["ls", "pwd", "echo", "cat", "grep", "find", "ps"]
dangerous_commands = ["rm", "dd", "mkfs", "chmod", "chown", "sudo"]
default_approval = "prompt"  # always, prompt, never
allow_sudo = false

[privacy]
enabled = true
redact_patterns = [
    "password[=:].*",
    "api[_-]?key[=:].*",
    "AWS_[A-Z_]+_KEY[=:].*",
]

[keybindings]
activate_ai = "ctrl-space"
copy_mode = "ctrl-a"
quit = "ctrl-q"

[terminal]
scrollback_lines = 10000
mouse_support = true

[context]
max_lines = 500  # How much terminal history to send to AI
max_size_kb = 50
include_env_vars = false
```

**Key Differences from mprocs:**
- ❌ No `procs:` section (no process management)
- ❌ No process-specific configs
- ✅ AI provider settings
- ✅ Safety/approval settings
- ✅ Privacy filters
- ✅ Simple keybindings

---

## Testing Strategy

### Unit Tests

**Coverage Target:** 80%+

**Test Files:**
- `src/llm/tests.rs` - LLM client tests (mocked APIs)
- `src/command/tests.rs` - Parser and validator tests
- `src/privacy/tests.rs` - Privacy filter tests
- `src/ai_proc/tests.rs` - AI process logic tests

**Example:**
```rust
#[tokio::test]
async fn test_llm_client_anthropic() {
    let client = LLMClient::new(Provider::Anthropic, "claude-3-5-sonnet");
    let response = client.send_message("Hello").await.unwrap();
    assert!(!response.is_empty());
}

#[test]
fn test_command_parser_extracts_bash_blocks() {
    let markdown = "```bash\nls -la\n```";
    let commands = CommandParser::extract_commands(markdown);
    assert_eq!(commands, vec!["ls -la"]);
}
```

---

### Integration Tests

**Scenarios:**
1. Start mprocs with AI enabled
2. Activate AI chat
3. Send message to AI
4. Receive streaming response
5. Extract and approve command
6. Verify command execution
7. Check terminal history context

---

## File Structure

```
termin.ai/
├── src/
│   ├── main.rs                 (mprocs, ~10 lines added)
│   ├── app.rs                  (mprocs, ~20 lines added)
│   ├── config.rs               (mprocs, ~30 lines added)
│   ├── protocol.rs             (mprocs, ~5 lines added)
│   ├── keymap.rs               (mprocs, ~10 lines added)
│   │
│   ├── llm/                    (NEW - LLM client)
│   │   ├── mod.rs
│   │   ├── client.rs
│   │   ├── providers.rs
│   │   ├── streaming.rs
│   │   ├── prompts.rs
│   │   └── tests.rs
│   │
│   ├── ai_proc/                (NEW - AI chat process)
│   │   ├── mod.rs
│   │   ├── chat_process.rs
│   │   ├── ui.rs
│   │   ├── context.rs
│   │   └── tests.rs
│   │
│   ├── command/                (NEW - Command parsing)
│   │   ├── mod.rs
│   │   ├── parser.rs
│   │   ├── validator.rs
│   │   ├── executor.rs
│   │   └── tests.rs
│   │
│   ├── privacy/                (NEW - Privacy filters)
│   │   ├── mod.rs
│   │   ├── filter.rs
│   │   └── tests.rs
│   │
│   └── [all other mprocs files unchanged]
│
├── MPROCS_PATCHES.md           (NEW - Track modifications)
├── ORIGINAL_*.md               (Preserved original docs)
├── README.md                   (Updated for Termin.AI)
└── Cargo.toml                  (mprocs + LLM dependencies)
```

---

## Dependencies

### Added to mprocs Cargo.toml

```toml
[dependencies]
# Existing mprocs dependencies...
# (all preserved)

# NEW: LLM client
genai = "0.3"

# NEW: Additional utilities for AI features
regex = "1.10"  # Already in mprocs
```

**Note:** Most dependencies already exist in mprocs!

---

## Benefits of This Approach

### 1. **Faster Development**
- Reuse proven PTY and VT100 implementation
- Focus on AI features and single-shell UX
- Skip months of terminal emulation debugging
- ~4-5 weeks instead of 8+ weeks from scratch

### 2. **Higher Quality Terminal Handling**
- mprocs' VT100 implementation is mature and tested
- Handles edge cases we'd miss starting from scratch
- 500+ GitHub stars, production-ready code
- Cross-platform PTY handling already works

### 3. **Freedom to Innovate on UX**
- Not constrained by mprocs' multi-process model
- Build exactly the single-shell experience we want
- AI overlay architecture designed from scratch
- Own our product vision completely

### 4. **Selective Improvement Integration**
- Monitor mprocs for terminal handling improvements
- Cherry-pick bug fixes as needed
- No obligation to track every mprocs change
- Focus only on relevant improvements

### 5. **Clear Project Identity**
- Termin.AI has distinct product vision
- Not "mprocs with AI" but "AI-assisted terminal"
- Users understand what we're building
- Community develops around our vision

---

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| mprocs breaking changes | Medium | Track specific version, document patches |
| Upstream API changes | Medium | Isolate integration points, abstract interfaces |
| Incompatible features | Low | All additions are optional/additive |
| Merge conflicts | Medium | Keep patches minimal and documented |
| Performance overhead | Low | AI features only active when enabled |

---

## Success Criteria

### Minimum Viable Product (v0.1.0)

- [x] mprocs codebase integrated
- [ ] LLM client working (Anthropic)
- [ ] AI chat process renders
- [ ] Can activate with Ctrl-Space
- [ ] Terminal context included
- [ ] Command parsing works
- [ ] Safety validation implemented
- [ ] <75 lines changed in mprocs core
- [ ] All mprocs features still work
- [ ] Documentation complete

### Future Enhancements (v0.2+)

- Multi-provider switching
- Voice input integration
- Team collaboration features
- Custom AI personalities
- Plugin system
- Enhanced safety rules

---

## Next Steps

### Immediate (This Week)

1. ✅ Integrate mprocs codebase
2. ✅ Verify mprocs builds
3. [ ] Test mprocs functionality
4. [ ] Create new module structure
5. [ ] Add LLM dependencies

### Next Week

1. [ ] Implement LLM client module
2. [ ] Test API connections
3. [ ] Implement streaming

### Following Weeks

Follow phases 2-8 as outlined above.

---

## Conclusion

By leveraging mprocs' terminal virtualization technology, we transform an 8-week ground-up implementation into a 4-5 week focused build. We get:

- ✅ Production-ready VT100 emulation (borrowed from mprocs)
- ✅ Proven PTY handling (borrowed from mprocs)
- ✅ Mature terminal rendering (borrowed from mprocs)
- ✅ Freedom to build our own product vision
- ✅ Clear single-shell + AI overlay architecture

**The path forward is clear: borrow proven terminal code, build unique product.**

### Key Takeaway

**Termin.AI is NOT:**
- A fork of mprocs
- An extension of mprocs
- "mprocs with AI added"

**Termin.AI IS:**
- A single-shell terminal with AI assistance
- Using mprocs' terminal virtualization as a library
- A distinct product with its own vision
- Building on proven technology to move faster

---

**Next Action:** Begin restructuring codebase for single-shell architecture while preserving borrowed mprocs modules (vt100, proc, term).
