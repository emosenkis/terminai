# Implementation Plan: Termin.AI
## Architecture and Development Roadmap

**Version:** 1.0
**Date:** 2025-10-23
**Language:** Rust
**Target:** Cross-platform (Linux, macOS, Windows/WSL)

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Technology Stack](#technology-stack)
3. [Module Breakdown](#module-breakdown)
4. [Data Flow](#data-flow)
5. [Development Phases](#development-phases)
6. [Testing Strategy](#testing-strategy)
7. [Deployment Plan](#deployment-plan)

---

## Architecture Overview

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         User Terminal                            │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                     Termin.AI Wrapper                            │
│                                                                   │
│  ┌───────────────┐  ┌──────────────┐  ┌─────────────────────┐  │
│  │   PTY Layer   │  │   Terminal   │  │   Overlay Manager   │  │
│  │   (Wrapper)   │◄─┤   History    │◄─┤   (UI Controller)   │  │
│  └───────┬───────┘  │   Buffer     │  └──────────┬──────────┘  │
│          │          └──────────────┘             │              │
│          │                                        │              │
│          │          ┌──────────────┐             │              │
│          │          │    Config    │             │              │
│          │          │   Manager    │             │              │
│          │          └──────────────┘             │              │
│          │                                        │              │
│          │          ┌──────────────┐             │              │
│          └─────────►│  Input       │◄────────────┘              │
│                     │  Handler     │                            │
│                     └──────┬───────┘                            │
│                            │                                     │
│                            ▼                                     │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              Event Loop (Tokio Async)                    │   │
│  └────────────┬──────────────────────────┬──────────────────┘   │
│               │                           │                      │
│               ▼                           ▼                      │
│  ┌─────────────────────┐    ┌──────────────────────────────┐   │
│  │   TUI Renderer      │    │    LLM Client Manager        │   │
│  │   (Ratatui)         │    │    (Multi-provider)          │   │
│  └─────────────────────┘    └──────────────┬───────────────┘   │
│                                             │                    │
└─────────────────────────────────────────────┼────────────────────┘
                                              │
                                              ▼
                              ┌──────────────────────────────┐
                              │   External LLM APIs          │
                              │ (Anthropic/OpenAI/Gemini)    │
                              └──────────────────────────────┘
```

### Component Interaction Flow

1. **User Input** → PTY Layer (passes through to shell)
2. **Special Key Combo** → Input Handler → Overlay Manager (activate UI)
3. **Terminal I/O** → History Buffer (capture context)
4. **Chat Input** → LLM Client Manager → API Request
5. **LLM Response** → Command Parser → Approval UI → Shell Execution
6. **Command Output** → Terminal & Chat Display

---

## Technology Stack

### Core Dependencies

| Category | Crate | Version | Purpose |
|----------|-------|---------|---------|
| **PTY Management** | `portable-pty` | 0.9.0 | Cross-platform PTY handling |
| **Async Runtime** | `tokio` | 1.35+ | Async I/O and event loop |
| **TUI Framework** | `ratatui` | 0.26+ | Terminal UI rendering |
| **Terminal Control** | `crossterm` | 0.27+ | Terminal manipulation (backend for ratatui) |
| **LLM Client** | `genai` | 0.3+ | Multi-provider LLM API client |
| **HTTP Client** | `reqwest` | 0.11+ | HTTP requests (via genai) |
| **Config** | `serde` + `toml` | latest | Configuration parsing |
| **Logging** | `tracing` + `tracing-subscriber` | latest | Structured logging |
| **Error Handling** | `anyhow` + `thiserror` | latest | Error management |
| **CLI Parsing** | `clap` | 4.4+ | Command-line argument parsing |

### Additional Dependencies

| Crate | Purpose |
|-------|---------|
| `regex` | Pattern matching for privacy filters |
| `dirs` | Platform-specific directory paths |
| `serde_json` | JSON serialization for history |
| `chrono` | Timestamp handling |
| `unicode-width` | Text width calculations for UI |
| `syntect` | Syntax highlighting for code blocks |
| `parking_lot` | High-performance locks |

---

## Module Breakdown

### 1. PTY Module (`src/pty/`)

**Purpose:** Wrap the user's shell in a pseudo-terminal and transparently pass I/O.

**Files:**
- `mod.rs` - Module exports
- `wrapper.rs` - PTY wrapper implementation
- `session.rs` - Shell session management
- `io.rs` - I/O handling and buffering

**Key Components:**

```rust
/// Main PTY wrapper that manages shell process
pub struct PtyWrapper {
    pty_system: Box<dyn PtySystem>,
    pty_pair: PtyPair,
    reader_handle: JoinHandle<()>,
    writer_handle: JoinHandle<()>,
    history_tx: mpsc::Sender<HistoryEvent>,
}

impl PtyWrapper {
    pub async fn new(shell: Shell, history_tx: mpsc::Sender<HistoryEvent>) -> Result<Self>;
    pub async fn run(&mut self) -> Result<()>;
    pub async fn write(&mut self, data: &[u8]) -> Result<()>;
    pub async fn resize(&mut self, rows: u16, cols: u16) -> Result<()>;
}

/// Represents the shell being wrapped
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    Sh,
    Custom(PathBuf),
}

impl Shell {
    pub fn detect() -> Result<Self>;
    pub fn command_path(&self) -> PathBuf;
    pub fn args(&self) -> Vec<String>;
}
```

**Responsibilities:**
- Spawn shell process in PTY
- Bidirectional I/O between user terminal and shell PTY
- Handle terminal resize events
- Forward signals (SIGINT, SIGTERM, etc.)
- Detect shell type automatically
- Stream I/O to history buffer

**Key Challenges:**
- Raw mode terminal handling
- Signal forwarding
- Handling terminal control sequences
- Ensuring zero-copy where possible

---

### 2. History Module (`src/history/`)

**Purpose:** Capture and manage terminal I/O history for LLM context.

**Files:**
- `mod.rs` - Module exports
- `buffer.rs` - Circular buffer implementation
- `filter.rs` - Privacy filtering
- `persistence.rs` - Save/load history

**Key Components:**

```rust
/// Terminal history buffer with privacy filtering
pub struct HistoryBuffer {
    entries: VecDeque<HistoryEntry>,
    max_lines: usize,
    max_bytes: usize,
    filters: Vec<PrivacyFilter>,
    current_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub timestamp: DateTime<Utc>,
    pub entry_type: EntryType,
    pub content: String,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone)]
pub enum EntryType {
    Command,
    Output,
    Error,
}

impl HistoryBuffer {
    pub fn new(config: &HistoryConfig) -> Self;
    pub fn push(&mut self, entry: HistoryEntry);
    pub fn get_context(&self, max_lines: usize) -> String;
    pub fn search(&self, query: &str) -> Vec<&HistoryEntry>;
    pub fn clear(&mut self);
    pub async fn save(&self, path: &Path) -> Result<()>;
    pub async fn load(path: &Path) -> Result<Self>;
}

/// Privacy filter for redacting sensitive information
pub struct PrivacyFilter {
    patterns: Vec<Regex>,
    replacement: String,
}

impl PrivacyFilter {
    pub fn new(patterns: Vec<String>) -> Result<Self>;
    pub fn apply(&self, text: &str) -> String;
    pub fn default_filters() -> Vec<Self>;
}
```

**Responsibilities:**
- Circular buffer for terminal I/O
- Automatic privacy filtering (passwords, API keys)
- Memory-efficient storage
- Serialization for persistence
- Context extraction for LLM
- Search functionality

**Privacy Filters (Default):**
```rust
const DEFAULT_PATTERNS: &[&str] = &[
    r"password[=:]\s*\S+",
    r"api[_-]?key[=:]\s*\S+",
    r"AWS_[A-Z_]+_KEY[=:]\s*\S+",
    r"token[=:]\s*\S+",
    r"secret[=:]\s*\S+",
    r"[a-f0-9]{64}", // Potential hash/key
];
```

---

### 3. UI Module (`src/ui/`)

**Purpose:** Render the overlay chat interface using Ratatui.

**Files:**
- `mod.rs` - Module exports
- `overlay.rs` - Main overlay window
- `chat.rs` - Chat message rendering
- `input.rs` - User input widget
- `theme.rs` - Color schemes and styling
- `approval.rs` - Command approval UI

**Key Components:**

```rust
/// Main overlay UI state
pub struct OverlayUI {
    visible: bool,
    messages: Vec<ChatMessage>,
    input: InputWidget,
    scroll_offset: u16,
    approval_state: Option<ApprovalState>,
    theme: Theme,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub tokens: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

impl OverlayUI {
    pub fn new(theme: Theme) -> Self;
    pub fn toggle_visibility(&mut self);
    pub fn add_message(&mut self, message: ChatMessage);
    pub fn render(&mut self, frame: &mut Frame);
    pub fn handle_input(&mut self, key: KeyEvent) -> UIAction;
}

/// User input widget with multi-line support
pub struct InputWidget {
    content: String,
    cursor: usize,
    history: Vec<String>,
    history_index: usize,
}

/// Command approval UI state
pub struct ApprovalState {
    pub command: String,
    pub risk_level: RiskLevel,
    pub selected: ApprovalOption,
}

pub enum ApprovalOption {
    Approve,
    Deny,
    Edit,
}

pub enum RiskLevel {
    Safe,      // Green - auto-approvable
    Caution,   // Yellow - prompt
    Dangerous, // Red - always confirm
}
```

**Responsibilities:**
- Overlay window rendering
- Message list with scrolling
- Input field with multi-line support
- Command approval dialog
- Syntax highlighting for code blocks
- Markdown rendering
- Keyboard navigation
- Theme support

**Layout Structure:**
```
┌─────────────────────────────────────────────────┐
│  Title Bar          [Tokens: 1234]    [x] Close │ 3 lines
├─────────────────────────────────────────────────┤
│                                                  │
│  Chat Messages (scrollable)                     │ 60% height
│  - User messages (right-aligned, blue)          │
│  - AI messages (left-aligned, green)            │
│  - Code blocks (syntax highlighted)             │
│                                                  │
├─────────────────────────────────────────────────┤
│  [Approval Dialog - if active]                  │ Auto height
│   Command: rm -rf /tmp/*                        │
│   Risk: CAUTION                                 │
│   > [Approve] [Deny] [Edit]                    │
├─────────────────────────────────────────────────┤
│  Input:                                         │ 20% height
│  █                                              │
│                                                  │
├─────────────────────────────────────────────────┤
│  Ctrl-Enter: Send | Esc: Close | Ctrl-E: Exec  │ 1 line
└─────────────────────────────────────────────────┘
```

---

### 4. LLM Module (`src/llm/`)

**Purpose:** Interface with multiple LLM providers via unified API.

**Files:**
- `mod.rs` - Module exports
- `client.rs` - Main LLM client manager
- `provider.rs` - Provider traits and implementations
- `streaming.rs` - Stream handling for responses
- `prompt.rs` - Prompt construction and templates

**Key Components:**

```rust
/// Multi-provider LLM client
pub struct LLMClient {
    genai_client: genai::Client,
    current_provider: Provider,
    current_model: String,
    config: LLMConfig,
}

#[derive(Debug, Clone)]
pub enum Provider {
    Anthropic,
    OpenAI,
    Gemini,
    Ollama,
}

impl LLMClient {
    pub async fn new(config: LLMConfig) -> Result<Self>;
    pub async fn send_message(&self, request: ChatRequest) -> Result<ChatResponse>;
    pub async fn stream_message(&self, request: ChatRequest) -> Result<impl Stream<Item = Result<String>>>;
    pub fn switch_provider(&mut self, provider: Provider, model: String) -> Result<()>;
}

/// Chat request with context
pub struct ChatRequest {
    pub message: String,
    pub terminal_context: TerminalContext,
    pub conversation_history: Vec<ChatMessage>,
    pub system_prompt: String,
}

#[derive(Debug, Clone)]
pub struct TerminalContext {
    pub history: String,
    pub cwd: PathBuf,
    pub shell: String,
    pub last_exit_code: Option<i32>,
    pub env_vars: HashMap<String, String>,
}

/// Streaming response handler
pub struct ResponseStream {
    inner: Pin<Box<dyn Stream<Item = Result<String>>>>,
}

impl ResponseStream {
    pub async fn next_chunk(&mut self) -> Option<Result<String>>;
    pub async fn collect_all(&mut self) -> Result<String>;
}
```

**System Prompt Template:**
```rust
const SYSTEM_PROMPT: &str = r#"
You are an AI assistant integrated into a terminal session. You help users with command-line tasks.

Current context:
- Working directory: {cwd}
- Shell: {shell}
- Last exit code: {exit_code}

Terminal history (last {history_lines} lines):
{history}

Guidelines:
1. When suggesting commands, format them in markdown code blocks with ```bash
2. Explain what commands do before suggesting them
3. Warn about destructive operations
4. For complex tasks, break them into steps
5. If unsure, ask for clarification
6. Consider the terminal history when answering

Response format:
- Use markdown for formatting
- Put commands in ```bash blocks
- Use **bold** for important warnings
- Keep responses concise but complete
"#;
```

**Responsibilities:**
- Unified API across providers (via `genai` crate)
- Request/response handling
- Streaming support
- Token counting and cost estimation
- Rate limiting
- Error handling and retries
- Context preparation
- Prompt engineering

---

### 5. Command Execution Module (`src/executor/`)

**Purpose:** Parse, validate, and execute commands suggested by LLM.

**Files:**
- `mod.rs` - Module exports
- `parser.rs` - Extract commands from LLM responses
- `validator.rs` - Safety checking and risk assessment
- `executor.rs` - Command execution in shell
- `approval.rs` - Approval workflow

**Key Components:**

```rust
/// Command executor with safety checks
pub struct CommandExecutor {
    pty_wrapper: Arc<Mutex<PtyWrapper>>,
    safety_config: SafetyConfig,
    approval_tx: mpsc::Sender<ApprovalRequest>,
}

/// Extracted command from LLM response
#[derive(Debug, Clone)]
pub struct Command {
    pub raw: String,
    pub parsed: Vec<String>,
    pub risk_level: RiskLevel,
    pub description: Option<String>,
}

impl CommandExecutor {
    pub fn new(pty_wrapper: Arc<Mutex<PtyWrapper>>, config: SafetyConfig) -> Self;
    pub async fn execute(&self, command: Command) -> Result<ExecutionResult>;
    pub fn parse_commands(llm_response: &str) -> Vec<Command>;
    pub fn assess_risk(command: &Command) -> RiskLevel;
}

/// Command parser for extracting bash code blocks
pub struct CommandParser;

impl CommandParser {
    pub fn extract_commands(markdown: &str) -> Vec<String>;
    pub fn parse_command(command: &str) -> Result<Vec<String>>;
}

/// Safety validator
pub struct SafetyValidator {
    safe_commands: HashSet<String>,
    dangerous_commands: HashSet<String>,
    patterns: Vec<DangerPattern>,
}

#[derive(Debug)]
pub struct DangerPattern {
    pub regex: Regex,
    pub description: String,
    pub risk: RiskLevel,
}

impl SafetyValidator {
    pub fn validate(&self, command: &Command) -> ValidationResult;
    pub fn default_rules() -> Self;
}

#[derive(Debug)]
pub struct ValidationResult {
    pub risk_level: RiskLevel,
    pub warnings: Vec<String>,
    pub blocked: bool,
}

/// Approval request sent to UI
pub struct ApprovalRequest {
    pub command: Command,
    pub response_tx: oneshot::Sender<ApprovalResponse>,
}

pub enum ApprovalResponse {
    Approved,
    Denied,
    Modified(String),
}
```

**Safety Rules:**
```rust
// Safe commands (can auto-approve if configured)
const SAFE_COMMANDS: &[&str] = &[
    "ls", "pwd", "echo", "cat", "grep", "find", "which",
    "head", "tail", "wc", "sort", "uniq", "cut", "sed",
    "awk", "diff", "less", "more", "file", "stat",
];

// Dangerous commands (always require approval)
const DANGEROUS_COMMANDS: &[&str] = &[
    "rm", "dd", "mkfs", "fdisk", "parted",
    "chmod", "chown", "kill", "killall",
    "reboot", "shutdown", "halt", "poweroff",
    "iptables", "ufw", "systemctl",
];

// Danger patterns
const DANGER_PATTERNS: &[(&str, &str)] = &[
    (r"sudo\s+", "Sudo command detected"),
    (r"rm\s+.*-[rf]", "Recursive/force delete"),
    (r"curl.*\|\s*bash", "Pipe to bash (security risk)"),
    (r">\s*/dev/", "Writing to device file"),
    (r"mkfs", "Filesystem creation"),
    (r"dd.*of=/dev/", "Direct disk write"),
];
```

**Responsibilities:**
- Parse commands from markdown code blocks
- Risk assessment
- Safety validation
- Approval workflow coordination
- Command execution via PTY
- Output capture
- Timeout handling
- Interactive command detection

---

### 6. Input Handler Module (`src/input/`)

**Purpose:** Capture and route keyboard input.

**Files:**
- `mod.rs` - Module exports
- `handler.rs` - Main input handler
- `bindings.rs` - Key binding configuration

**Key Components:**

```rust
/// Input handler for routing key events
pub struct InputHandler {
    key_bindings: KeyBindings,
    mode: InputMode,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputMode {
    PassThrough,  // Normal terminal operation
    Overlay,      // Chat overlay active
}

pub struct KeyBindings {
    activation_key: KeyBinding,
    custom_bindings: HashMap<KeyBinding, Action>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyBinding {
    pub key: KeyCode,
    pub modifiers: KeyModifiers,
}

#[derive(Debug, Clone)]
pub enum Action {
    ActivateOverlay,
    CloseOverlay,
    SendMessage,
    ApproveCommand,
    DenyCommand,
    EditCommand,
    ClearHistory,
    SwitchProvider,
}

impl InputHandler {
    pub fn new(config: &InputConfig) -> Self;
    pub fn handle_key(&mut self, key: KeyEvent) -> InputAction;
    pub fn set_mode(&mut self, mode: InputMode);
}

pub enum InputAction {
    PassToShell(KeyEvent),
    PassToOverlay(KeyEvent),
    ToggleOverlay,
    Execute(Action),
    None,
}
```

**Default Key Bindings:**
```rust
const DEFAULT_BINDINGS: &[(KeyBinding, Action)] = &[
    (KeyBinding::new(KeyCode::Char(' '), KeyModifiers::CONTROL), Action::ActivateOverlay),
    (KeyBinding::new(KeyCode::Esc, KeyModifiers::NONE), Action::CloseOverlay),
    (KeyBinding::new(KeyCode::Enter, KeyModifiers::CONTROL), Action::SendMessage),
    (KeyBinding::new(KeyCode::Char('y'), KeyModifiers::NONE), Action::ApproveCommand),
    (KeyBinding::new(KeyCode::Char('n'), KeyModifiers::NONE), Action::DenyCommand),
    (KeyBinding::new(KeyCode::Char('e'), KeyModifiers::NONE), Action::EditCommand),
];
```

**Responsibilities:**
- Capture all keyboard input
- Route input based on mode
- Handle activation key combo
- Support configurable bindings
- Conflict detection

---

### 7. Configuration Module (`src/config/`)

**Purpose:** Load, validate, and manage configuration.

**Files:**
- `mod.rs` - Module exports
- `loader.rs` - Config file loading
- `schema.rs` - Configuration schema
- `validation.rs` - Config validation

**Key Components:**

```rust
/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub ui: UIConfig,
    pub context: ContextConfig,
    pub execution: ExecutionConfig,
    pub llm: LLMConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub shell: ShellOption,
    pub log_level: LogLevel,
    pub history_file: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIConfig {
    pub activation_key: String,
    pub overlay_height_percent: u8,
    pub overlay_width_percent: u8,
    pub theme: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    pub max_lines: usize,
    pub max_size_kb: usize,
    pub include_env_vars: bool,
    pub redact_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    pub safe_commands: Vec<String>,
    pub dangerous_commands: Vec<String>,
    pub default_approval: ApprovalMode,
    pub allow_sudo: bool,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfig {
    pub default_provider: String,
    pub default_model: String,
    pub stream_responses: bool,
    pub max_tokens: usize,
    pub temperature: f32,
    pub providers: HashMap<String, ProviderConfig>,
}

impl Config {
    pub fn load() -> Result<Self>;
    pub fn from_file(path: &Path) -> Result<Self>;
    pub fn validate(&self) -> Result<()>;
    pub fn default_path() -> PathBuf;
}
```

**Configuration Search Path:**
1. `./terminai.toml` (current directory)
2. `~/.config/terminai/config.toml`
3. `/etc/terminai/config.toml`
4. Built-in defaults

**Responsibilities:**
- Load config from multiple sources
- TOML parsing and deserialization
- Schema validation
- Default value handling
- Hot-reload support (future)
- Environment variable overrides

---

### 8. Event Loop Module (`src/event_loop/`)

**Purpose:** Central async event loop coordinating all components.

**Files:**
- `mod.rs` - Module exports
- `loop.rs` - Main event loop
- `events.rs` - Event types

**Key Components:**

```rust
/// Main application event loop
pub struct EventLoop {
    pty_wrapper: Arc<Mutex<PtyWrapper>>,
    history_buffer: Arc<Mutex<HistoryBuffer>>,
    overlay_ui: Arc<Mutex<OverlayUI>>,
    llm_client: Arc<LLMClient>,
    command_executor: Arc<CommandExecutor>,
    input_handler: Arc<Mutex<InputHandler>>,
    event_rx: mpsc::Receiver<Event>,
    shutdown_tx: broadcast::Sender<()>,
}

/// Application events
#[derive(Debug)]
pub enum Event {
    // Input events
    KeyPress(KeyEvent),
    MouseEvent(MouseEvent),

    // PTY events
    ShellOutput(Vec<u8>),
    ShellExit(i32),

    // History events
    HistoryUpdate(HistoryEntry),

    // UI events
    OverlayToggle,
    MessageSend(String),

    // LLM events
    LLMResponse(String),
    LLMError(String),
    StreamChunk(String),

    // Execution events
    CommandApproval(ApprovalRequest),
    CommandExecute(Command),
    CommandComplete(ExecutionResult),

    // System events
    Resize(u16, u16),
    Shutdown,
}

impl EventLoop {
    pub async fn new(config: Config) -> Result<Self>;
    pub async fn run(mut self) -> Result<()>;

    async fn handle_event(&mut self, event: Event) -> Result<()>;
    async fn handle_key_press(&mut self, key: KeyEvent) -> Result<()>;
    async fn handle_message_send(&mut self, message: String) -> Result<()>;
    async fn handle_llm_response(&mut self, response: String) -> Result<()>;
    async fn handle_command_approval(&mut self, request: ApprovalRequest) -> Result<()>;
}
```

**Event Flow:**
```
User Input → InputHandler → Event → EventLoop → Route to component
Shell Output → PTY → HistoryBuffer → Event → EventLoop → UI Update
Chat Input → UI → Event → EventLoop → LLM Client → Event → UI
LLM Response → Parser → Command → Event → Executor → Event → Shell
```

**Responsibilities:**
- Central event coordination
- Async task spawning
- Error propagation and handling
- Graceful shutdown
- Component lifecycle management

---

### 9. Utilities Module (`src/utils/`)

**Purpose:** Shared utilities and helpers.

**Files:**
- `mod.rs` - Module exports
- `terminal.rs` - Terminal detection and control
- `syntax.rs` - Syntax highlighting
- `markdown.rs` - Markdown rendering
- `paths.rs` - Path manipulation

---

## Data Flow

### 1. Normal Terminal Operation

```
User Keystroke
    ↓
InputHandler (PassThrough mode)
    ↓
PTY Wrapper
    ↓
Shell Process
    ↓
Shell Output
    ↓
PTY Wrapper → HistoryBuffer (background)
    ↓
User Terminal
```

### 2. Overlay Activation

```
User presses Ctrl-Space
    ↓
InputHandler detects activation key
    ↓
Event: OverlayToggle
    ↓
EventLoop → OverlayUI
    ↓
UI renders overlay
    ↓
InputHandler switches to Overlay mode
```

### 3. Chat Message Flow

```
User types message in overlay
    ↓
User presses Ctrl-Enter
    ↓
Event: MessageSend
    ↓
EventLoop → LLM Client
    ├─ Prepare context from HistoryBuffer
    ├─ Build chat request
    └─ Send to API
    ↓
Stream response chunks
    ↓
Event: StreamChunk (multiple)
    ↓
EventLoop → OverlayUI
    ↓
Render streaming response
    ↓
Parse for commands
    ↓
If command found → Event: CommandApproval
```

### 4. Command Execution Flow

```
Command parsed from LLM response
    ↓
Event: CommandApproval
    ↓
EventLoop → CommandExecutor
    ├─ Assess risk level
    └─ SafetyValidator
    ↓
If dangerous → Show approval UI
    ↓
User approves/denies/edits
    ↓
If approved → Event: CommandExecute
    ↓
CommandExecutor → PTY Wrapper
    ↓
Write command to shell
    ↓
Capture output
    ↓
Event: CommandComplete
    ↓
Display result in UI and terminal
```

---

## Development Phases

### Phase 1: Foundation (Weeks 1-2)

**Goal:** Basic PTY wrapper that transparently passes I/O

**Tasks:**
- [ ] Project setup (Cargo.toml, directory structure)
- [ ] Implement PTY wrapper with `portable-pty`
- [ ] Shell detection and spawning
- [ ] Bidirectional I/O pass-through
- [ ] Terminal resize handling
- [ ] Signal forwarding
- [ ] Basic logging with `tracing`
- [ ] Manual testing with bash and zsh

**Deliverable:** Binary that wraps a shell transparently

**Testing:**
```bash
cargo run -- bash
# Should behave identically to running bash directly
# Test: vim, htop, tab completion, history, Ctrl-C, etc.
```

---

### Phase 2: History Capture (Week 3)

**Goal:** Capture terminal I/O into a history buffer

**Tasks:**
- [ ] Implement circular buffer for history
- [ ] Parse I/O into commands vs output
- [ ] Privacy filter implementation
- [ ] Default redaction patterns
- [ ] Memory management (cap size)
- [ ] Context extraction API
- [ ] Unit tests for buffer and filters

**Deliverable:** History buffer captures terminal session

**Testing:**
```rust
#[test]
fn test_history_buffer() {
    let mut buffer = HistoryBuffer::new(config);
    buffer.push(HistoryEntry { ... });
    let context = buffer.get_context(50);
    assert!(context.contains("ls -la"));
}
```

---

### Phase 3: Configuration (Week 3)

**Goal:** Load and validate configuration from TOML

**Tasks:**
- [ ] Define configuration schema
- [ ] Implement config loader
- [ ] Search path handling
- [ ] Default values
- [ ] Validation rules
- [ ] Unit tests for config parsing

**Deliverable:** Working configuration system

---

### Phase 4: TUI Overlay (Week 4)

**Goal:** Render basic overlay using Ratatui

**Tasks:**
- [ ] Setup Ratatui + Crossterm
- [ ] Implement overlay window
- [ ] Message list rendering
- [ ] Input widget with cursor
- [ ] Scrolling support
- [ ] Theme implementation
- [ ] Integration with event loop

**Deliverable:** Overlay that can display mock messages

**Testing:**
- Press Ctrl-Space to open
- Type messages (not sent yet)
- Scroll through message list
- Press Esc to close

---

### Phase 5: Input Handling (Week 4)

**Goal:** Route input between terminal and overlay

**Tasks:**
- [ ] Implement InputHandler
- [ ] Key binding system
- [ ] Mode switching (PassThrough vs Overlay)
- [ ] Activation key detection
- [ ] Integration with PTY and UI

**Deliverable:** Seamless input routing

---

### Phase 6: LLM Integration (Week 5)

**Goal:** Send messages to LLM and receive responses

**Tasks:**
- [ ] Integrate `genai` crate
- [ ] Implement LLMClient
- [ ] Provider configuration (Anthropic, OpenAI)
- [ ] API key management
- [ ] Request/response handling
- [ ] Streaming support
- [ ] Error handling
- [ ] Context preparation with terminal history

**Deliverable:** Working chat with LLM

**Testing:**
```bash
# Open overlay
# Type: "What files are in this directory?"
# LLM should see terminal history (ls output)
# LLM should respond appropriately
```

---

### Phase 7: Command Parsing (Week 5)

**Goal:** Extract and parse commands from LLM responses

**Tasks:**
- [ ] Markdown code block parser
- [ ] Command extraction
- [ ] Multi-line command support
- [ ] Shell syntax validation
- [ ] Unit tests for various formats

**Deliverable:** Reliable command extraction

---

### Phase 8: Safety & Approval (Week 6)

**Goal:** Implement command safety checks and approval UI

**Tasks:**
- [ ] SafetyValidator implementation
- [ ] Risk assessment rules
- [ ] Approval UI component
- [ ] Approval workflow
- [ ] Command editing support
- [ ] Integration with executor

**Deliverable:** Safe command execution with approval

---

### Phase 9: Command Execution (Week 6)

**Goal:** Execute approved commands in the shell

**Tasks:**
- [ ] CommandExecutor implementation
- [ ] Write commands to PTY
- [ ] Output capture
- [ ] Timeout handling
- [ ] Interactive command detection
- [ ] Result display in UI

**Deliverable:** End-to-end command execution

**Testing:**
```bash
# Ask LLM: "Create a test directory"
# LLM suggests: mkdir test_dir
# Approve command
# Verify directory created
# Output visible in both terminal and chat
```

---

### Phase 10: Polish & Testing (Week 7)

**Goal:** Bug fixes, error handling, edge cases

**Tasks:**
- [ ] Comprehensive error handling
- [ ] Graceful degradation
- [ ] Edge case testing
- [ ] Memory leak detection
- [ ] Performance profiling
- [ ] Cross-platform testing (Linux, macOS)
- [ ] Shell compatibility testing (bash, zsh, fish)

**Deliverable:** Stable, production-ready build

---

### Phase 11: Documentation (Week 8)

**Goal:** Complete documentation and examples

**Tasks:**
- [ ] README with quick start
- [ ] Configuration guide
- [ ] Architecture documentation
- [ ] API documentation (rustdoc)
- [ ] Example configurations
- [ ] Troubleshooting guide
- [ ] Demo video/GIF

**Deliverable:** Complete documentation

---

### Phase 12: Release (Week 8)

**Goal:** Public release

**Tasks:**
- [ ] Final testing
- [ ] Version tagging
- [ ] Release binaries (GitHub Releases)
- [ ] Cargo publish
- [ ] Announcement (blog post, social media)
- [ ] Community setup (Discord, issues)

---

## Testing Strategy

### Unit Tests

**Coverage Target:** 80%+

**Focus Areas:**
- History buffer operations
- Privacy filtering
- Command parsing
- Safety validation
- Configuration parsing
- Input handling

**Example:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_privacy_filter_redacts_api_key() {
        let filter = PrivacyFilter::default_filters();
        let input = "export API_KEY=sk-1234567890abcdef";
        let output = filter.apply(input);
        assert!(output.contains("[REDACTED]"));
        assert!(!output.contains("sk-1234567890abcdef"));
    }

    #[test]
    fn test_command_parser_extracts_bash_blocks() {
        let markdown = r#"
        You can use this command:
        ```bash
        ls -la /tmp
        ```
        "#;
        let commands = CommandParser::extract_commands(markdown);
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], "ls -la /tmp");
    }
}
```

---

### Integration Tests

**Test Scenarios:**
1. **Full workflow test:** Start wrapper → open overlay → chat → execute command → verify output
2. **Multi-provider test:** Switch between OpenAI and Anthropic
3. **Approval workflow:** Dangerous command requires approval
4. **History persistence:** Restart wrapper, history loads correctly
5. **Error recovery:** LLM API fails, graceful fallback

**Example:**
```rust
#[tokio::test]
async fn test_end_to_end_workflow() {
    let config = Config::default();
    let app = EventLoop::new(config).await.unwrap();

    // Simulate key press: Ctrl-Space
    app.send_event(Event::KeyPress(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL)));

    // Verify overlay opened
    assert!(app.overlay_ui.lock().await.visible);

    // Send chat message
    app.send_event(Event::MessageSend("List files".to_string()));

    // Wait for LLM response
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Verify command parsed and displayed
    // ...
}
```

---

### Manual Testing Checklist

**Pre-release Testing:**

- [ ] Install on fresh Linux machine
- [ ] Install on macOS
- [ ] Test with bash 4.x and 5.x
- [ ] Test with zsh
- [ ] Test with fish
- [ ] Run vim, verify it works correctly
- [ ] Run htop, verify it works correctly
- [ ] Test tab completion
- [ ] Test command history (Ctrl-R)
- [ ] Test terminal resize
- [ ] Test Ctrl-C to interrupt commands
- [ ] Test with long-running commands (sleep 60)
- [ ] Test with colored output (ls --color)
- [ ] Test with unicode characters
- [ ] Test API failures (disconnect network)
- [ ] Test with various terminal emulators (Alacritty, iTerm2, GNOME Terminal)
- [ ] Test sudo commands with approval
- [ ] Test rm commands require confirmation
- [ ] Test privacy filter (export API_KEY=xxx)
- [ ] Test conversation persistence
- [ ] Test config file changes

---

## Deployment Plan

### Build Process

**Release Builds:**
```bash
# Linux x86_64
cargo build --release --target x86_64-unknown-linux-gnu

# macOS Intel
cargo build --release --target x86_64-apple-darwin

# macOS ARM
cargo build --release --target aarch64-apple-darwin

# Static binary (Linux)
cargo build --release --target x86_64-unknown-linux-musl
```

**Optimizations:**
```toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true
```

---

### Distribution

**1. GitHub Releases**
- Attach binaries for each platform
- Include checksums (SHA256)
- Provide installation script

**2. Cargo**
```bash
cargo publish
```

**3. Package Managers (Future)**
- Homebrew formula (macOS/Linux)
- AUR package (Arch Linux)
- Debian package (.deb)
- RPM package (Fedora/RHEL)

---

### Installation Script

```bash
#!/bin/bash
# install.sh

set -e

OS=$(uname -s)
ARCH=$(uname -m)

if [ "$OS" = "Darwin" ]; then
    if [ "$ARCH" = "arm64" ]; then
        BINARY_URL="https://github.com/user/terminai/releases/latest/download/terminai-macos-arm64"
    else
        BINARY_URL="https://github.com/user/terminai/releases/latest/download/terminai-macos-x64"
    fi
elif [ "$OS" = "Linux" ]; then
    BINARY_URL="https://github.com/user/terminai/releases/latest/download/terminai-linux-x64"
else
    echo "Unsupported OS: $OS"
    exit 1
fi

echo "Downloading Termin.AI..."
curl -L "$BINARY_URL" -o /tmp/terminai
chmod +x /tmp/terminai
sudo mv /tmp/terminai /usr/local/bin/terminai

echo "Termin.AI installed successfully!"
echo "Run 'terminai' to start."
```

---

### Quick Start Documentation

**README.md:**
```markdown
# Termin.AI

Interactive terminal with integrated LLM assistance.

## Installation

```bash
curl -sSL https://raw.githubusercontent.com/user/terminai/main/install.sh | bash
```

## Quick Start

1. Set your API key:
   ```bash
   export ANTHROPIC_API_KEY=your_key_here
   ```

2. Start Termin.AI:
   ```bash
   terminai
   ```

3. Use your terminal normally

4. Press Ctrl-Space to open AI chat

5. Ask for help, execute suggested commands

## Configuration

Create `~/.config/terminai/config.toml`:

```toml
[llm]
default_provider = "anthropic"
default_model = "claude-3-5-sonnet-20241022"
```

See [Configuration Guide](docs/configuration.md) for details.
```

---

## Project Structure

```
terminai/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── PRD.md
├── IMPLEMENTATION_PLAN.md
├── LICENSE
├── .gitignore
│
├── src/
│   ├── main.rs                 # Entry point
│   ├── lib.rs                  # Library root
│   │
│   ├── pty/                    # PTY wrapper module
│   │   ├── mod.rs
│   │   ├── wrapper.rs
│   │   ├── session.rs
│   │   └── io.rs
│   │
│   ├── history/                # History buffer module
│   │   ├── mod.rs
│   │   ├── buffer.rs
│   │   ├── filter.rs
│   │   └── persistence.rs
│   │
│   ├── ui/                     # UI module
│   │   ├── mod.rs
│   │   ├── overlay.rs
│   │   ├── chat.rs
│   │   ├── input.rs
│   │   ├── approval.rs
│   │   └── theme.rs
│   │
│   ├── llm/                    # LLM client module
│   │   ├── mod.rs
│   │   ├── client.rs
│   │   ├── provider.rs
│   │   ├── streaming.rs
│   │   └── prompt.rs
│   │
│   ├── executor/               # Command execution module
│   │   ├── mod.rs
│   │   ├── parser.rs
│   │   ├── validator.rs
│   │   ├── executor.rs
│   │   └── approval.rs
│   │
│   ├── input/                  # Input handling module
│   │   ├── mod.rs
│   │   ├── handler.rs
│   │   └── bindings.rs
│   │
│   ├── config/                 # Configuration module
│   │   ├── mod.rs
│   │   ├── loader.rs
│   │   ├── schema.rs
│   │   └── validation.rs
│   │
│   ├── event_loop/             # Event loop module
│   │   ├── mod.rs
│   │   ├── loop.rs
│   │   └── events.rs
│   │
│   └── utils/                  # Utilities module
│       ├── mod.rs
│       ├── terminal.rs
│       ├── syntax.rs
│       ├── markdown.rs
│       └── paths.rs
│
├── tests/                      # Integration tests
│   ├── integration_test.rs
│   ├── pty_test.rs
│   └── command_test.rs
│
├── benches/                    # Benchmarks
│   └── history_bench.rs
│
├── examples/                   # Example code
│   ├── basic_usage.rs
│   └── custom_config.rs
│
├── docs/                       # Documentation
│   ├── configuration.md
│   ├── architecture.md
│   ├── troubleshooting.md
│   └── images/
│
└── scripts/                    # Build/deploy scripts
    ├── install.sh
    ├── build-release.sh
    └── test-all.sh
```

---

## Dependencies (Cargo.toml)

```toml
[package]
name = "terminai"
version = "0.1.0"
edition = "2021"
authors = ["Your Name <email@example.com>"]
description = "Interactive terminal with integrated LLM assistance"
license = "MIT OR Apache-2.0"
repository = "https://github.com/user/terminai"
keywords = ["terminal", "llm", "ai", "shell", "assistant"]
categories = ["command-line-utilities", "development-tools"]

[dependencies]
# Async runtime
tokio = { version = "1.35", features = ["full"] }
tokio-stream = "0.1"

# PTY handling
portable-pty = "0.9"

# TUI
ratatui = "0.26"
crossterm = { version = "0.27", features = ["event-stream"] }

# LLM client
genai = "0.3"
reqwest = { version = "0.11", features = ["json", "stream"] }

# Configuration
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"

# Error handling
anyhow = "1.0"
thiserror = "1.0"

# CLI
clap = { version = "4.4", features = ["derive", "env"] }

# Utilities
regex = "1.10"
dirs = "5.0"
chrono = { version = "0.4", features = ["serde"] }
unicode-width = "0.1"
parking_lot = "0.12"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Syntax highlighting
syntect = "5.1"

[dev-dependencies]
tempfile = "3.8"
criterion = "0.5"
pretty_assertions = "1.4"

[[bench]]
name = "history_bench"
harness = false

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true
panic = "abort"

[profile.dev]
opt-level = 0
debug = true
```

---

## Risk Mitigation

### Risk: PTY handling complexity

**Mitigation:**
- Use well-tested `portable-pty` crate (from wezterm)
- Extensive testing with different shells
- Fallback to direct shell execution if PTY fails

### Risk: LLM API costs

**Mitigation:**
- Support local models via Ollama
- Configurable rate limiting
- Token usage tracking and warnings
- Caching for repeated queries

### Risk: Command execution security

**Mitigation:**
- Multi-level approval system
- Default to safe mode (all commands need approval)
- Extensive validation rules
- User education about risks

### Risk: Performance overhead

**Mitigation:**
- Async I/O for non-blocking operations
- Efficient buffering
- Benchmarking and profiling
- Zero-copy where possible

### Risk: Terminal compatibility

**Mitigation:**
- Test on major terminal emulators
- Use crossterm for cross-platform support
- Document known issues
- Graceful degradation

---

## Success Metrics

**Technical Metrics:**
- Startup time: <100ms
- Keystroke latency: <1ms (99th percentile)
- Memory usage: <50MB base
- Test coverage: >80%
- Zero critical bugs in first month

**User Metrics:**
- 1000+ downloads in first month
- 10+ GitHub stars in first week
- Positive feedback from beta testers
- Feature requests indicate product-market fit

---

## Next Steps

1. **Week 1:** Set up project structure, implement basic PTY wrapper
2. **Week 2:** Complete PTY wrapper with full I/O handling
3. **Week 3:** History buffer and configuration system
4. **Week 4:** TUI overlay and input routing
5. **Week 5:** LLM integration and command parsing
6. **Week 6:** Safety system and command execution
7. **Week 7:** Testing and polish
8. **Week 8:** Documentation and release

**Let's build this! 🚀**
