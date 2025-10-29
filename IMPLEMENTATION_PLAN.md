# Termin.AI Implementation Plan
## Building on mprocs Foundation

**Version:** 2.0 (Revised based on mprocs integration)
**Date:** 2025-10-29
**Base:** mprocs v0.7.3
**Strategy:** Minimal changes to mprocs, maximal upstream compatibility

---

## Executive Summary

After analyzing the mprocs codebase, we've discovered it already provides 90% of the infrastructure needed for Termin.AI:

- ✅ PTY handling (via portable-pty)
- ✅ TUI interface (via ratatui)
- ✅ Terminal I/O capture
- ✅ Process management
- ✅ Keyboard input routing
- ✅ Configuration system (YAML)
- ✅ Copy mode
- ✅ Remote control API

**New Strategy:** Add LLM assistance as a **minimally-invasive extension** to mprocs, preserving the ability to merge upstream improvements with minimal conflicts.

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

## What We Need to Add

### Core Addition: LLM Assistant Module

**Primary Requirement:** Add LLM chat functionality with minimal changes to existing mprocs code.

**Key Features to Add:**
1. LLM chat interface (new process type)
2. Multi-provider LLM client
3. Terminal history context extraction
4. Command suggestion and execution
5. Safety/approval system
6. Privacy filters

**Design Principle:** Add functionality as **new modules** and **new process types** rather than modifying existing mprocs code.

---

## Architecture: Extension Strategy

### Design Philosophy

**Goal:** Treat mprocs as an upstream dependency that we extend, not fork.

**Approach:**
1. **Add, Don't Modify:** Create new modules alongside existing ones
2. **Extend, Don't Replace:** Use mprocs' extension points (new process types, commands)
3. **Isolate LLM Code:** Keep AI-specific code in separate modules
4. **Preserve Interfaces:** Don't change existing mprocs APIs
5. **Document Patches:** Minimal necessary changes clearly marked for easy upstream merging

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Termin.AI Application                     │
│                                                               │
│  ┌────────────────────────────────────────────────────────┐ │
│  │                   mprocs Core                           │ │
│  │                (Unmodified or Minimal Changes)          │ │
│  │                                                          │ │
│  │  ┌──────────┐  ┌──────────┐  ┌────────────┐           │ │
│  │  │   PTY    │  │   TUI    │  │   Config   │           │ │
│  │  │  Engine  │  │  Engine  │  │   System   │           │ │
│  │  └──────────┘  └──────────┘  └────────────┘           │ │
│  │                                                          │ │
│  │  ┌──────────┐  ┌──────────┐  ┌────────────┐           │ │
│  │  │  Process │  │  Kernel  │  │   Events   │           │ │
│  │  │  Manager │  │  (Async) │  │  & Input   │           │ │
│  │  └──────────┘  └──────────┘  └────────────┘           │ │
│  └────────────────────────────────────────────────────────┘ │
│                             │                                 │
│                             ▼                                 │
│  ┌────────────────────────────────────────────────────────┐ │
│  │              Termin.AI Extensions (NEW)                 │ │
│  │                                                          │ │
│  │  ┌──────────┐  ┌──────────┐  ┌────────────┐           │ │
│  │  │   LLM    │  │  AI Chat │  │  Command   │           │ │
│  │  │  Client  │  │ Process  │  │  Parser    │           │ │
│  │  └──────────┘  └──────────┘  └────────────┘           │ │
│  │                                                          │ │
│  │  ┌──────────┐  ┌──────────┐  ┌────────────┐           │ │
│  │  │  Safety  │  │ Privacy  │  │  History   │           │ │
│  │  │ Validator│  │  Filter  │  │  Context   │           │ │
│  │  └──────────┘  └──────────┘  └────────────┘           │ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
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

## Integration Points

### Minimal Changes to mprocs Core

**1. Add AI Process Type** (`src/app.rs` - ~20 lines)

```rust
// Add after existing process creation
use crate::ai_proc::AIChatProcess;

// In create_app_proc() function:
if config.enable_ai {
    let ai_proc = AIChatProcess::new(config.ai_config);
    // Register as special process
}
```

**2. Add AI Config** (`src/config.rs` - ~30 lines)

```rust
#[derive(Deserialize)]
pub struct AIConfig {
    pub enabled: bool,
    pub provider: String,
    pub model: String,
    pub activation_key: Option<String>,
}

// Add to Config struct:
pub ai: Option<AIConfig>,
```

**3. Add Activation Key Binding** (`src/keymap.rs` - ~10 lines)

```rust
// Add command variant:
pub enum Command {
    // ... existing commands
    ToggleAI,
}

// Add to default keymap:
keymap.insert(KeyCode::Char(' '), Command::ToggleAI);
```

**4. Add AI Command to Protocol** (`src/protocol.rs` - ~5 lines)

```rust
pub enum ClientCommand {
    // ... existing commands
    ToggleAI,
    SendAIMessage { message: String },
}
```

**Total mprocs Changes:** ~65 lines across 4 files.

---

## Development Phases

### Phase 1: Setup & Foundation (Week 1)

**Goal:** Verify mprocs builds and runs, set up project structure

**Tasks:**
- [x] Clone mprocs repository
- [x] Verify cargo build works
- [x] Run mprocs and test basic functionality
- [ ] Create new module directories
- [ ] Add LLM dependencies to Cargo.toml
- [ ] Set up testing framework

**Deliverable:** mprocs builds and runs with new dependencies added

---

### Phase 2: LLM Client Module (Week 2)

**Goal:** Standalone LLM client that works independently

**Tasks:**
- [ ] Implement `src/llm/client.rs` using genai crate
- [ ] Add support for Anthropic Claude
- [ ] Add support for OpenAI GPT-4
- [ ] Implement streaming responses
- [ ] Write unit tests
- [ ] Create example/test binary

**Deliverable:** LLM client can chat with AI providers

**Testing:**
```bash
# Test LLM client independently
cargo test --package terminai --lib llm::tests
```

---

### Phase 3: Command Parser & Safety (Week 3)

**Goal:** Parse and validate commands from LLM responses

**Tasks:**
- [ ] Implement markdown code block parser
- [ ] Implement SafetyValidator with risk levels
- [ ] Add privacy filter
- [ ] Create command approval UI component
- [ ] Write comprehensive tests

**Deliverable:** Can extract and classify commands safely

---

### Phase 4: AI Chat Process (Week 4)

**Goal:** AI process that renders in mprocs UI

**Tasks:**
- [ ] Create AIChatProcess struct
- [ ] Implement chat UI rendering
- [ ] Add input handling
- [ ] Implement context extraction from other processes
- [ ] Test as standalone process in mprocs

**Deliverable:** AI chat renders in mprocs alongside other processes

---

### Phase 5: Integration (Week 5)

**Goal:** Connect AI process to mprocs kernel

**Tasks:**
- [ ] Add AI config to mprocs config system
- [ ] Add activation key binding
- [ ] Integrate AI process with process manager
- [ ] Add remote control commands for AI
- [ ] Test full workflow

**Deliverable:** Can activate AI chat with key press in running mprocs

---

### Phase 6: Command Execution (Week 6)

**Goal:** Execute approved commands in target processes

**Tasks:**
- [ ] Implement command executor
- [ ] Add process selection for command target
- [ ] Implement approval workflow
- [ ] Handle command output streaming
- [ ] Add error handling

**Deliverable:** AI can suggest and execute commands with approval

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

## Upstream Compatibility

### Strategy for Minimizing Merge Conflicts

**1. Isolate Changes**
- All AI-specific code in new modules
- Changes to mprocs core clearly marked with comments:
  ```rust
  // TERMIN.AI: Start of AI integration
  ...
  // TERMIN.AI: End of AI integration
  ```

**2. Use Extension Points**
- Add new commands to protocol (doesn't modify existing)
- Add new process types (doesn't modify existing)
- Add new config sections (doesn't modify existing)

**3. Document Patches**
- Maintain `MPROCS_PATCHES.md` documenting all changes
- Track mprocs version: v0.7.3
- Note reasons for each modification

**4. Regular Upstream Syncs**
- Fetch mprocs updates monthly
- Test integration with new versions
- Update patches as needed

**5. Contribute Back**
- Identify generally useful changes
- Submit PRs to mprocs for non-AI features
- Collaborate with mprocs maintainer

### Patch Documentation Template

```markdown
## MPROCS_PATCHES.md

### Version: mprocs v0.7.3

### Patch 1: AI Config Support
**File:** src/config.rs
**Lines:** 150-180
**Reason:** Add AI configuration section
**Upstream Impact:** None (additive only)
**Merge Strategy:** Copy section to new versions

### Patch 2: Toggle AI Command
**File:** src/protocol.rs
**Lines:** 45-47
**Reason:** Add ToggleAI command variant
**Upstream Impact:** Low (adds to enum)
**Merge Strategy:** Add variant to updated enum
```

---

## Configuration

### Extended mprocs.yaml

```yaml
# Standard mprocs configuration
procs:
  server:
    shell: "npm run dev"
  tests:
    shell: "npm test"

# Termin.AI extensions
ai:
  enabled: true
  provider: "anthropic"
  model: "claude-3-5-sonnet-20241022"
  activation_key: "<C-Space>"

  providers:
    anthropic:
      api_key_env: "ANTHROPIC_API_KEY"
    openai:
      api_key_env: "OPENAI_API_KEY"

  # Safety settings
  safety:
    safe_commands: ["ls", "pwd", "cat", "grep"]
    dangerous_commands: ["rm", "dd", "chmod"]
    default_approval: "prompt"

  # Privacy
  privacy:
    enabled: true
    redact_patterns:
      - "password=.*"
      - "api[_-]?key=.*"
```

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
- Skip PTY, TUI, config implementation
- Focus only on AI-specific features
- 3-4 weeks vs 8+ weeks

### 2. **Better Quality**
- Built on mature, tested codebase
- mprocs has 500+ GitHub stars
- Well-designed architecture

### 3. **Upstream Benefits**
- Pull in mprocs improvements
- Bug fixes and features
- Community contributions

### 4. **Minimal Maintenance**
- Only maintain AI-specific code
- mprocs handles core functionality
- Easier to keep up-to-date

### 5. **Clear Separation**
- AI code isolated in modules
- Easy to disable AI features
- Can use as pure mprocs

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

By building on mprocs, we transform an 8-week ground-up implementation into a 4-week focused extension. We get:

- ✅ Production-ready PTY and TUI foundation
- ✅ Proven terminal management
- ✅ Mature process architecture
- ✅ Active upstream development
- ✅ Clear path to adding AI features

**The path forward is clear: extend, don't rebuild.**

---

**Next Action:** Verify mprocs builds with `cargo build` and test basic functionality.
