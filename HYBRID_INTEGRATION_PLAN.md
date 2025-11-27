# Hybrid Terminal Integration Plan
## Integrating HybridTerminal into terminai Binary

**Version:** 1.0
**Date:** 2025-11-27
**Target:** `src/bin/terminai.rs`
**Scope:** Replace existing implementation with hybrid terminal system

---

## Executive Summary

The terminai binary currently implements a basic single-shell terminal with AI overlay using direct vt100 integration and a simple event loop. This plan outlines replacing it with the sophisticated hybrid terminal system that provides:

- **4-mode state machine** (Passthrough, GuestAltBuffer, ModalWithBuffering, ModalGuestAlt)
- **Automatic buffer management** for seamless mode transitions
- **Output buffering and replay** when modal closes
- **Proper alt buffer detection** and handling

### Current State (terminai.rs)
- ✅ Single shell with PTY management
- ✅ AI overlay with Ctrl-Space toggle
- ✅ Direct vt100 parser usage
- ✅ Basic event loop with 60 FPS rendering
- ⚠️ No output buffering during modal display
- ⚠️ No proper mode management
- ⚠️ Manual host alt buffer management

### Target State (After Integration)
- ✅ All current functionality preserved
- ✅ Hybrid terminal with sophisticated mode management
- ✅ Output buffering during modal display with replay
- ✅ Automatic host/guest buffer synchronization
- ✅ Better separation of concerns
- ✅ Cleaner, more maintainable code

---

## Architecture Comparison

### Current Architecture (terminai.rs)

```
┌─────────────────────────────────────────────────────────────┐
│                      App (main struct)                       │
├─────────────────────────────────────────────────────────────┤
│  ┌────────────┐  ┌──────────────┐  ┌──────────────────────┐ │
│  │ Terminal   │  │ Shell        │  │ AIChatProcess        │ │
│  │ (ratatui)  │  │ (vt100 + PTY)│  │ (AI state)           │ │
│  └────────────┘  └──────────────┘  └──────────────────────┘ │
│                                                               │
│  Event Loop:                                                  │
│  - Shell events → process → render                           │
│  - Keyboard → route based on ai_visible flag                 │
│  - Periodic render (60 FPS)                                   │
└─────────────────────────────────────────────────────────────┘
```

### New Architecture (HybridTerminal)

```
┌─────────────────────────────────────────────────────────────┐
│                      App (main struct)                       │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────────────────────────────────────────────┐   │
│  │           HybridTerminal                              │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌────────────┐ │   │
│  │  │ ModeManager  │  │ OutputRouter │  │ Renderer   │ │   │
│  │  │ ShadowTerm   │  │ HostControl  │  │ Buffer     │ │   │
│  │  └──────────────┘  └──────────────┘  └────────────┘ │   │
│  └──────────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ AIModalAdapter                                        │   │
│  │  - Wraps AIChatProcess                                │   │
│  │  - Converts to/from ModalState                        │   │
│  │  - Handles command execution                          │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                               │
│  PTY Management (Shell → channels → HybridTerminal)          │
└─────────────────────────────────────────────────────────────┘
```

---

## Integration Components

### Component 1: PTY-to-Channel Bridge

**Purpose:** Bridge between PTY (std::thread) and HybridTerminal (tokio channels)

**Location:** New struct in `terminai.rs`

**Responsibilities:**
- Spawn PTY using portable-pty
- Read PTY output in thread, send to channel
- Receive input from channel, write to PTY
- Handle PTY lifecycle (resize, exit)

**Interface:**
```rust
struct PtyBridge {
    pty_output_tx: UnboundedSender<Vec<u8>>,
    pty_input_rx: UnboundedReceiver<Vec<u8>>,
    master: Box<dyn MasterPty + Send>,
    // ... exit handling
}

impl PtyBridge {
    fn spawn(shell_cmd: &str, rows: u16, cols: u16) -> Result<(
        Self,
        UnboundedReceiver<Vec<u8>>,  // PTY output for HybridTerminal
        UnboundedSender<Vec<u8>>,    // PTY input from HybridTerminal
    )>;

    fn resize(&mut self, rows: u16, cols: u16) -> Result<()>;
}
```

### Component 2: Reply Sender Implementation

**Purpose:** Implement TermReplySender for terminal queries

**Location:** New struct in `terminai.rs`

**Implementation:**
```rust
#[derive(Clone)]
struct TerminalReplySender {
    pty_input_tx: UnboundedSender<Vec<u8>>,
}

impl TermReplySender for TerminalReplySender {
    fn send(&self, data: Vec<u8>) {
        let _ = self.pty_input_tx.send(data);
    }
}
```

### Component 3: AI Modal Adapter

**Purpose:** Adapt AIChatProcess to work with HybridTerminal's ModalState

**Location:** New module `src/hybrid/ai_modal.rs` or in `terminai.rs`

**Responsibilities:**
- Convert AIChatProcess to ModalState for rendering
- Handle AI input (text input, command approval)
- Extract terminal context from HybridTerminal's shadow
- Execute approved commands via PTY input channel

**Interface:**
```rust
struct AIModalAdapter {
    ai_process: AIChatProcess,
    context_extractor: ContextExtractor,
    pty_input_tx: UnboundedSender<Vec<u8>>,
}

impl AIModalAdapter {
    fn to_modal_state(&self) -> ModalState;

    fn handle_input(&mut self, key: KeyEvent) -> Result<InputResult>;

    async fn send_message(&mut self, shadow: &ShadowTerminal<_>) -> Result<()>;

    fn execute_approved_command(&mut self) -> Result<()>;
}

enum InputResult {
    Consumed,
    PassThrough,
    CloseModal,
}
```

### Component 4: Main App Struct Refactor

**Purpose:** Simplify App to coordinate HybridTerminal and AI

**Responsibilities:**
- Create and manage HybridTerminal
- Create and manage AIModalAdapter
- Bridge app events (toggle AI, resize, quit)
- Coordinate AI modal state updates

**Structure:**
```rust
struct App {
    hybrid_terminal: HybridTerminal<TerminalReplySender>,
    ai_adapter: Option<AIModalAdapter>,
    app_event_tx: UnboundedSender<AppEvent>,
    pty_bridge: PtyBridge,
}
```

---

## Implementation Phases

### Phase 1: PTY Bridge (No Breaking Changes)

**Goal:** Create PTY-to-channel bridge without changing existing code

**Tasks:**
1. Create `PtyBridge` struct
2. Implement PTY spawning with channels
3. Implement resize forwarding
4. Implement exit detection
5. Add unit tests

**Validation:**
- PTY spawns correctly
- Output flows through channel
- Input flows to PTY
- Resize works
- Exit detected

**Files Modified:**
- `src/bin/terminai.rs` (add PtyBridge at end)

**Estimated Lines:** +150

---

### Phase 2: AI Modal Adapter (Parallel Development)

**Goal:** Create adapter between AIChatProcess and ModalState

**Tasks:**
1. Create AIModalAdapter struct
2. Implement `to_modal_state()` conversion
3. Implement input handling
4. Implement context extraction
5. Implement command execution
6. Add unit tests

**Validation:**
- AI process converts to ModalState correctly
- Input routing works
- Context extraction works
- Command execution works

**Files Modified:**
- `src/bin/terminai.rs` OR new `src/hybrid/ai_modal.rs`

**Estimated Lines:** +200

---

### Phase 3: HybridTerminal Integration (Breaking Changes)

**Goal:** Replace existing event loop with HybridTerminal

**Tasks:**
1. Create TerminalReplySender implementation
2. Refactor App struct to use HybridTerminal
3. Wire PTY channels to HybridTerminal
4. Create app event channel
5. Remove old event loop code
6. Remove old rendering code

**Validation:**
- Terminal renders correctly
- Shell I/O works
- Ctrl-Space toggles modal (basic)
- ESC closes modal
- Resize works

**Files Modified:**
- `src/bin/terminai.rs` (major refactor)

**Estimated Lines:** -200, +100 (net -100)

---

### Phase 4: AI Integration (Feature Complete)

**Goal:** Wire AI modal adapter into HybridTerminal

**Tasks:**
1. Integrate AIModalAdapter with HybridTerminal
2. Update modal state on AI changes
3. Handle AI input routing
4. Implement context extraction from shadow terminal
5. Test command approval and execution
6. Test streaming responses

**Validation:**
- AI modal displays correctly
- AI input works (typing, enter, backspace)
- Context extraction includes terminal history
- Command approval works (Y/N)
- Commands execute in shell
- Streaming responses display

**Files Modified:**
- `src/bin/terminai.rs`

**Estimated Lines:** +150

---

### Phase 5: Polish and Testing (Final)

**Goal:** Clean up, optimize, and thoroughly test

**Tasks:**
1. Remove all dead code
2. Add comprehensive error handling
3. Add logging for debugging
4. Test edge cases:
   - Modal during shell alt buffer (vim, htop)
   - Large output during modal
   - Rapid modal toggle
   - Long-running AI responses
5. Update comments and documentation
6. Performance testing

**Validation:**
- All features work correctly
- No memory leaks
- No performance regressions
- Clean code, good error messages

**Files Modified:**
- `src/bin/terminai.rs`

**Estimated Lines:** +50 (error handling, logging)

---

## Detailed Implementation Steps

### Step 1: Create PtyBridge

```rust
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

struct PtyBridge {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    output_tx: UnboundedSender<Vec<u8>>,
    input_rx: UnboundedReceiver<Vec<u8>>,
    exit_code: Arc<RwLock<Option<u32>>>,
}

impl PtyBridge {
    fn spawn(
        shell_cmd: &str,
        rows: u16,
        cols: u16,
    ) -> Result<(
        Self,
        UnboundedReceiver<Vec<u8>>, // For HybridTerminal
        UnboundedSender<Vec<u8>>,   // For HybridTerminal
    )> {
        let (output_tx, output_rx) = unbounded_channel();
        let (input_tx, input_rx) = unbounded_channel();

        // Create PTY
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })?;

        // Spawn shell
        let mut cmd = CommandBuilder::new(shell_cmd);
        cmd.env("TERM", "xterm-256color");
        let child = pair.slave.spawn_command(cmd)?;

        // Spawn reader thread
        let mut reader = pair.master.try_clone_reader()?;
        let output_tx_clone = output_tx.clone();
        std::thread::spawn(move || {
            let mut buf = vec![0u8; 32 * 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let _ = output_tx_clone.send(buf[..n].to_vec());
                    }
                    Err(_) => break,
                }
            }
        });

        // Spawn writer thread
        let mut writer_clone = pair.master.take_writer()?;
        tokio::spawn(async move {
            while let Some(data) = input_rx.recv().await {
                let _ = writer_clone.write_all(&data);
                let _ = writer_clone.flush();
            }
        });

        // Spawn exit watcher
        let exit_code = Arc::new(RwLock::new(None));
        let exit_code_clone = exit_code.clone();
        std::thread::spawn(move || {
            if let Ok(status) = child.wait() {
                *exit_code_clone.write().unwrap() = Some(status.exit_code());
            }
        });

        Ok((
            Self {
                master: pair.master,
                writer,
                output_tx,
                input_rx,
                exit_code,
            },
            output_rx,
            input_tx,
        ))
    }

    fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
        self.master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })?;
        Ok(())
    }

    fn check_exit(&self) -> Option<u32> {
        *self.exit_code.read().unwrap()
    }
}
```

### Step 2: Create AIModalAdapter

```rust
struct AIModalAdapter {
    ai_process: AIChatProcess,
    pty_input_tx: UnboundedSender<Vec<u8>>,
}

impl AIModalAdapter {
    fn to_modal_state(&self) -> ModalState {
        // Convert AI conversation to modal text
        let mut content = String::new();

        for msg in self.ai_process.conversation() {
            match msg.role {
                MessageRole::User => {
                    content.push_str(&format!("You: {}\n\n", msg.content));
                }
                MessageRole::Assistant => {
                    content.push_str(&format!("AI: {}\n\n", msg.content));
                }
                MessageRole::System => {
                    // Skip system messages
                }
            }
        }

        // Show input buffer
        content.push_str(&format!("\n> {}", self.ai_process.input_buffer()));

        // Show pending command if any
        if let Some(pending) = self.ai_process.pending_command() {
            content.push_str(&format!(
                "\n\n[Command: {}]\nApprove? (y/n): ",
                pending.command
            ));
        }

        ModalState::text("AI Assistant", content)
    }

    async fn handle_input(&mut self, key: KeyEvent) -> Result<InputResult> {
        // If there's a pending command, handle approval
        if self.ai_process.pending_command().is_some() {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.execute_approved_command()?;
                    return Ok(InputResult::Consumed);
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.ai_process.reject_command();
                    return Ok(InputResult::Consumed);
                }
                _ => return Ok(InputResult::Consumed), // Ignore other keys
            }
        }

        // Normal input handling
        match key.code {
            KeyCode::Char(c) if key.modifiers.is_empty() => {
                self.ai_process.append_input(&c.to_string());
                Ok(InputResult::Consumed)
            }
            KeyCode::Backspace => {
                self.ai_process.delete_char();
                Ok(InputResult::Consumed)
            }
            KeyCode::Enter => {
                // Signal that message should be sent (caller extracts context)
                Ok(InputResult::SendMessage)
            }
            KeyCode::Esc => Ok(InputResult::CloseModal),
            _ => Ok(InputResult::PassThrough),
        }
    }

    async fn send_message(&mut self, context: TerminalContext) -> Result<()> {
        self.ai_process.send_input_with_context(context).await
    }

    fn execute_approved_command(&mut self) -> Result<()> {
        if let Some(pending) = self.ai_process.approve_command() {
            // Send command to PTY
            let command_bytes = pending.command.as_bytes().to_vec();
            self.pty_input_tx.send(command_bytes)?;

            // Send Enter
            self.pty_input_tx.send(vec![b'\r'])?;
        }
        Ok(())
    }
}

enum InputResult {
    Consumed,
    PassThrough,
    CloseModal,
    SendMessage,
}
```

### Step 3: Refactor Main App

```rust
struct App {
    hybrid_terminal: HybridTerminal<TerminalReplySender>,
    ai_adapter: Option<AIModalAdapter>,
    pty_bridge: PtyBridge,
    app_event_rx: UnboundedReceiver<AppEvent>,
}

impl App {
    async fn new(shell_cmd: String) -> Result<Self> {
        // Get terminal size
        let (cols, rows) = crossterm::terminal::size()?;

        // Create PTY bridge
        let (pty_bridge, pty_output_rx, pty_input_tx) =
            PtyBridge::spawn(&shell_cmd, rows, cols)?;

        // Create app event channel
        let (app_event_tx, app_event_rx) = unbounded_channel();

        // Create reply sender
        let reply_sender = TerminalReplySender {
            pty_input_tx: pty_input_tx.clone(),
        };

        // Create hybrid terminal
        let hybrid_terminal = HybridTerminal::new(
            cols,
            rows,
            1000, // scrollback
            reply_sender,
            pty_output_rx,
            pty_input_tx.clone(),
            app_event_rx,
        )?;

        // Initialize AI if available
        let ai_adapter = Self::init_ai(pty_input_tx).await;

        Ok(Self {
            hybrid_terminal,
            ai_adapter,
            pty_bridge,
            app_event_rx,
        })
    }

    async fn run(mut self) -> Result<()> {
        // Run hybrid terminal
        self.hybrid_terminal.run().await?;
        Ok(())
    }
}
```

---

## Testing Strategy

### Unit Tests

**PtyBridge:**
- ✅ PTY spawns successfully
- ✅ Channels created correctly
- ✅ Resize forwards to PTY

**AIModalAdapter:**
- ✅ Converts to ModalState correctly
- ✅ Handles input appropriately
- ✅ Executes commands correctly

### Integration Tests

1. **Basic Terminal Operation**
   - Shell spawns and runs
   - Commands execute
   - Output displays correctly

2. **Modal Toggle**
   - Ctrl-Space shows modal
   - ESC closes modal
   - Output buffering works

3. **AI Functionality**
   - AI modal displays
   - Input works
   - Context extraction works
   - Commands execute

4. **Edge Cases**
   - Modal during vim (guest alt buffer)
   - Large output during modal
   - Rapid toggle
   - Long AI responses

### Manual Testing Checklist

- [ ] Shell launches correctly
- [ ] Commands execute and display output
- [ ] Ctrl-Space toggles AI modal
- [ ] ESC closes AI modal
- [ ] AI chat works (send message, get response)
- [ ] Command approval works (Y/N)
- [ ] Commands execute in shell after approval
- [ ] vim works correctly
- [ ] htop works correctly
- [ ] Resize works in all modes
- [ ] Output appears when modal closes
- [ ] No visible lag or performance issues

---

## Risk Assessment

### High Risk
- **PTY threading deadlock**: Mitigated by using channels and separate threads
- **Mode transition race conditions**: Mitigated by Arc<RwLock<>> and careful lock ordering

### Medium Risk
- **Output buffering overflow**: Mitigated by bounded buffer with overflow handling
- **AI response during mode change**: Mitigated by atomic state transitions

### Low Risk
- **Terminal rendering artifacts**: Mitigated by proper frame synchronization
- **Command execution timing**: Mitigated by sequential channel sends

---

## Success Criteria

### Functional
- ✅ All existing terminai features work
- ✅ Modal properly buffers output
- ✅ Output replays when modal closes
- ✅ Alt buffer detection works
- ✅ No crashes or panics

### Performance
- ✅ No perceivable lag in normal operation
- ✅ Modal toggle <100ms
- ✅ Rendering smooth at 60 FPS

### Code Quality
- ✅ Clean separation of concerns
- ✅ Well-documented code
- ✅ Comprehensive error handling
- ✅ Unit tests pass

---

## Rollback Plan

If integration fails or causes issues:

1. **Keep original terminai.rs as backup**: `terminai.rs.backup`
2. **Rollback commits**: Use git to revert to pre-integration state
3. **Feature flag**: Add conditional compilation if needed
4. **Gradual rollout**: Keep both implementations temporarily

---

## Timeline Estimate

| Phase | Description | Estimated Time |
|-------|-------------|----------------|
| 1 | PTY Bridge | 1-2 hours |
| 2 | AI Modal Adapter | 2-3 hours |
| 3 | HybridTerminal Integration | 2-3 hours |
| 4 | AI Integration | 2-3 hours |
| 5 | Polish and Testing | 2-3 hours |
| **Total** | | **9-14 hours** |

---

## Next Steps

1. ✅ Review and approve this plan
2. Create progress tracking file
3. Backup current terminai.rs
4. Begin Phase 1: PTY Bridge
5. Iterative development with testing after each phase

---

## Notes

- This integration is **focused only on terminai binary**
- Does **not** modify the main mprocs-based codebase
- Preserves all existing AI functionality
- Adds sophisticated mode management
- Improves code maintainability
