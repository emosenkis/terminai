# Code Borrowed from mprocs v0.7.3

**Base Version:** mprocs v0.7.3
**Repository:** https://github.com/pvolok/mprocs
**License:** MIT
**Integration Date:** 2025-10-29
**Strategy Clarified:** 2025-11-14

---

## Overview

This document tracks code borrowed from mprocs for Termin.AI. We are **NOT maintaining a fork** or **extending mprocs as a product**. Instead, we're using mprocs' excellent terminal virtualization code as a **library foundation** for our own distinct product.

**Key Distinction:**
- **mprocs:** Multi-process manager with config-driven tabs
- **Termin.AI:** Single shell with AI overlay

**Relationship:**
- Like using `tokio` for async or `ratatui` for TUI
- We borrow proven terminal virtualization technology
- We build our own product architecture on top

---

## Modules Borrowed (Reused As-Is or Lightly Modified)

### 1. `src/vt100/` - VT100 Terminal Emulation

**Files:**
- `term.rs` - Terminal emulator core
- `screen.rs` - Screen buffer management
- `cell.rs` - Terminal cell data structure
- `grid.rs` - Grid implementation
- `parser.rs` - VT100 escape sequence parser
- `attrs.rs` - Text attributes (bold, colors, etc.)
- `row.rs` - Terminal row implementation

**Status:** ✅ Used as-is
**Modifications:** Minimal (import paths only)
**Reason:** Excellent, mature VT100 implementation
**Value:** Handles complex terminal emulation edge cases we'd miss

**Update Strategy:**
- Monitor mprocs for VT100 parser bug fixes
- Cherry-pick escape sequence handling improvements
- Watch for Unicode/emoji handling updates

---

### 2. `src/proc/` - PTY Management

**Files:**
- `proc.rs` - Process management (simplified for single shell)
- `inst.rs` - Process instance management
- `msg.rs` - Process messages

**Status:** 🔧 Modified
**Modifications:**
- Removed multi-process kernel dependencies
- Simplified to single-process usage
- Kept core PTY handling logic

**Reason:** Production-ready PTY handling
**Value:** Cross-platform PTY management is complex

**Update Strategy:**
- Monitor for PTY handling improvements
- Watch for signal handling fixes
- Cherry-pick platform-specific bug fixes

---

### 3. `src/term/` - Terminal Abstractions

**Files:**
- `term.rs` - Terminal trait and implementations
- Related terminal utilities

**Status:** ✅ Used as-is
**Modifications:** Minor (imports)
**Reason:** Clean terminal abstractions
**Value:** Well-designed interface

**Update Strategy:**
- Monitor for API improvements
- Watch for rendering optimizations

---

### 4. `src/widgets/` - UI Components

**Files:**
- Reusable widget components
- Layout helpers

**Status:** 🔧 Partially used
**Modifications:** Selected widgets reused, some replaced
**Reason:** Some widgets applicable to our UI
**Value:** Don't reinvent basic UI components

**Update Strategy:**
- Selectively review widget improvements
- Focus on generally useful components

---

### 5. Input Handling

**Files:**
- `src/key.rs` - Keyboard input types
- `src/event.rs` - Event system

**Status:** ✅ Used as-is
**Modifications:** Minimal
**Reason:** Good input handling abstraction
**Value:** Cross-platform keyboard support

**Update Strategy:**
- Monitor for key handling fixes
- Watch for new key binding features

---

## Modules Replaced (New Termin.AI Code)

### 1. `src/main.rs` - Application Entry Point

**Status:** ❌ Completely replaced
**Why:** Different entry point - launch single shell, not multi-process manager
**Similarity:** 0%

**New Implementation:**
```rust
// Termin.AI main.rs
// - Detect user's shell ($SHELL)
// - Launch single shell process
// - Initialize AI assistant
// - Run single-shell app
```

---

### 2. `src/app.rs` - Application Core

**Status:** ❌ Completely replaced
**Why:** Different architecture - single shell + AI overlay, not multi-process tabs
**Similarity:** ~5% (basic event loop structure)

**New Implementation:**
```rust
// Termin.AI app.rs
pub struct App {
    shell_proc: ShellProcess,  // Single shell
    terminal: Term,              // VT100 (from mprocs)
    ai: Option<AIAssistant>,     // AI overlay
    mode: AppMode,               // Normal/AI/Copy
}
```

---

### 3. `src/config.rs` - Configuration

**Status:** ❌ Completely replaced
**Why:** Different config - AI settings, not process management
**Similarity:** 0%

**New Implementation:**
```toml
# Termin.AI config
[ai]
provider = "anthropic"
model = "claude-3-5-sonnet-20241022"

[safety]
safe_commands = [...]
dangerous_commands = [...]

# NO process management config!
```

---

### 4. `src/ui_*.rs` - UI Layouts

**Status:** ❌ Mostly replaced
**Why:** Different UI - single terminal + overlay, not process list + tabs
**Similarity:** ~20% (some rendering patterns)

**New Files:**
- `src/ui_shell.rs` - Full-screen terminal display
- `src/ui_ai_overlay.rs` - AI chat overlay
- `src/ui_copy.rs` - Copy mode (adapted from mprocs)

---

### 5. `src/kernel/` - Multi-Process Kernel

**Status:** ❌ Removed/Not used
**Why:** We have a single shell, not multiple processes
**Similarity:** N/A

---

### 6. `src/server/`, `src/client.rs` - Remote Control

**Status:** ❌ Not used (initially)
**Why:** Not needed for v1.0
**Future:** May add remote control later

---

## New Termin.AI Modules

### AI-Specific Code (Not from mprocs)

1. **`src/llm/`** - LLM Client
   - Multi-provider API client (Anthropic, OpenAI, Gemini, Ollama)
   - Streaming response handling
   - API key management

2. **`src/ai/`** - AI Assistant
   - Conversation management
   - Context extraction from terminal
   - AI overlay rendering
   - User interaction

3. **`src/command/`** - Command Handling
   - Parse commands from LLM markdown responses
   - Safety validation (safe/dangerous commands)
   - Command injection into shell PTY
   - Approval workflow

4. **`src/privacy/`** - Privacy Filter
   - Redact sensitive patterns (passwords, API keys)
   - Terminal history sanitization

---

## Code Statistics

### Borrowed from mprocs

| Category | Lines (est.) | Percentage |
|----------|--------------|------------|
| VT100 emulation | ~2000 | 15% |
| PTY handling | ~800 | 6% |
| Terminal abstractions | ~400 | 3% |
| Input handling | ~300 | 2% |
| Widgets (partial) | ~500 | 4% |
| **Total Borrowed** | **~4000** | **~30%** |

### New Termin.AI Code

| Category | Lines (est.) | Percentage |
|----------|--------------|------------|
| App/Main/Config | ~800 | 6% |
| UI (shell + overlay) | ~1200 | 9% |
| LLM client | ~1000 | 8% |
| AI assistant | ~1500 | 11% |
| Command handling | ~1000 | 8% |
| Privacy/Safety | ~500 | 4% |
| Tests/Examples | ~3000 | 23% |
| **Total New** | **~9000** | **~70%** |

---

## Upstream Monitoring Strategy

### High Priority (Monitor Closely)

**VT100 Parsing:**
- Escape sequence handling bugs
- Unicode/emoji edge cases
- Scrollback issues
- Color/attribute rendering

**PTY Handling:**
- Signal handling improvements
- Platform-specific fixes (Linux/macOS/BSD)
- Process cleanup edge cases
- I/O buffer management

### Medium Priority (Review Occasionally)

**Input Handling:**
- New key binding features
- Mouse support improvements
- Clipboard integration

**Rendering:**
- Performance optimizations
- TUI widget improvements

### Low Priority (Informational)

**Multi-Process Features:**
- Process management improvements (not relevant)
- Config system changes (we use different config)
- Remote control features (not using)

---

## Cherry-Picking Workflow

### When mprocs Releases New Version

1. **Review Release Notes:**
   ```bash
   # Check mprocs releases
   git remote add mprocs https://github.com/pvolok/mprocs.git
   git fetch mprocs --tags
   ```

2. **Identify Relevant Changes:**
   - Focus on `src/vt100/`, `src/proc/`, `src/term/`
   - Look for bug fixes, not new features
   - Check for security fixes

3. **Selectively Cherry-Pick:**
   ```bash
   # Example: cherry-pick VT100 fix
   git cherry-pick <commit-hash> -- src/vt100/parser.rs
   # Resolve conflicts
   # Test thoroughly
   ```

4. **Test Integration:**
   - `cargo build` - Verify it compiles
   - `cargo test` - Run tests
   - Manual testing - Verify terminal handling works
   - AI features still work

5. **Document Update:**
   - Update this file with new mprocs version
   - Note what was cherry-picked
   - Record any issues encountered

---

## Contributing Back to mprocs

### Bug Fixes to Submit Upstream

If we discover bugs in borrowed code:

**VT100 Parsing:**
- Edge cases in escape sequence handling
- Unicode rendering issues
- Scrollback bugs

**PTY Handling:**
- Signal handling edge cases
- Platform-specific issues
- Resource cleanup bugs

**Process:**
1. Create minimal reproduction
2. Verify bug exists in mprocs main
3. Open issue in mprocs repo
4. Submit PR with fix
5. Credit both projects

### Collaboration Opportunities

**Shared Library Idea:**
Consider proposing `terminal-virt` library:
- Both Termin.AI and mprocs depend on it
- VT100 + PTY handling extracted
- Reduces code duplication
- Benefits both projects

---

## Update History

### v0.7.3 (Base Integration)
- **Date:** 2025-10-29
- **Status:** Initial integration
- **Borrowed:** VT100, PTY, term, widgets
- **Modified:** 0% (used as-is initially)

### Strategy Clarification
- **Date:** 2025-11-14
- **Change:** Clarified we're not extending mprocs, but borrowing code
- **Impact:** Freedom to restructure for single-shell architecture
- **Plan:** Replace app/config/UI, keep VT100/PTY

### [Future Updates]
- Next update will document first restructuring
- Will track modifications to borrowed modules
- Will record any upstream cherry-picks

---

## Questions & Contact

**Using borrowed code:**
- See source code comments for details
- Most VT100/PTY code unchanged from mprocs
- Check git history for our modifications

**Contributing to mprocs:**
- Open issues for bugs found
- Submit PRs for fixes
- Credit both projects

**License:**
- mprocs is MIT licensed
- Termin.AI is MIT licensed
- Proper attribution maintained

---

## Attribution

**mprocs by Pavel Volokitin**
- Repository: https://github.com/pvolok/mprocs
- License: MIT
- Used with permission under MIT license

**Termin.AI Team:**
- Product: Distinct from mprocs
- Usage: Library-style code reuse
- License: MIT (compatible)
- Attribution: This document + LICENSE file

---

**Last Updated:** 2025-11-14
**mprocs Version:** v0.7.3
**Termin.AI Version:** v0.1.0-dev
**Relationship:** Code borrowing (not fork, not extension)
