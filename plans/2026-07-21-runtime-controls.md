# Runtime AI Controls Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add session-level approval modes, safe internal-history clearing, and confirmed switching between configured CLI agents through overlay-only hotkeys and a control-panel TUI.

**Architecture:** Keep runtime policy and modal state in the existing `AppState`, route all agent suggestions through its existing queue consumer, and reuse agent launch-plan construction for switching. Add one VT primitive that discards only non-visible rows, plus one small UI module for the control panel, picker, and confirmations.

**Tech Stack:** Rust 2024, Ratatui, Crossterm/crokey, Serde YAML/Schemars, portable-pty, existing VT100 parser and rat-salsa event loop.

---

### Task 1: Configuration types and switcher inventory

**Files:**
- Modify: `src/terminai_config.rs`
- Modify: `src/agent_launcher.rs`
- Modify: `config/codex.yaml`
- Modify: `config/claude.yaml`
- Modify: `config/opencode.yaml`

**Steps:**

1. Add failing config tests proving `approval-mode` defaults to `always-ask`, accepts `auto-approval`, the four new keybindings default to F7–F10, and `show-in-switcher` defaults to true.
2. Run `cargo test -p termin terminai_config::tests --lib` and confirm the new assertions fail.
3. Add a serializable `ApprovalMode` enum, the top-level `approval_mode` field, backward-compatible defaults for the four keybindings, and `show_in_switcher` on `AgentPresetConfig`.
4. Add an `available_agent_presets` helper in `agent_launcher.rs` that merges bundled and user preset names, filters hidden entries, sorts names, and keeps bundled presets visible by default.
5. Add focused helper tests and run `cargo test -p termin agent_launcher::tests terminai_config::tests --lib`.

### Task 2: Internal scrollback truncation

**Files:**
- Modify: `src/vt100/grid.rs`
- Modify: `src/vt100/screen.rs`
- Modify: `src/vt100/parser.rs`
- Modify: `src/tests/e2e/test_scrollback.rs`

**Steps:**

1. Add a failing test that produces more than one viewport of output, clears internal history, and asserts that visible rows are unchanged, `total_rows()` equals the viewport height, scrollback offset is zero, and pending native rows are empty.
2. Run the focused test and confirm it fails because no clear operation exists.
3. Implement `Grid::clear_scrollback` by dropping rows before `row0`, resetting `scrollback_offset`, and clearing `pending_native_scrollback`; expose it through `Screen` and `Parser`.
4. Run the focused test and the existing scrollback suite.

### Task 3: Approval routing and overlay status

**Files:**
- Modify: `src/bin/terminai.rs`
- Modify: `src/tests/e2e/test_ai.rs`

**Steps:**

1. Extract a small pure helper that decides whether a pending suggestion is queued or immediately sent from `ApprovalMode`; test that auto-approval ignores `RiskLevel` and always-ask queues every level.
2. Add `approval_mode` to `AppState`, initialize it from config, and update `process_agent_suggestions` to send all auto-approved input through the existing shell writer path without opening the approval dialog.
3. Add a rendering helper/test for the overlay title/status line and render right-aligned yellow/red `⚠ AUTO-APPROVE` only in auto mode.
4. Run the focused binary and AI UI tests.

### Task 4: Control panel and overlay-only shortcuts

**Files:**
- Create: `src/ui_controls.rs`
- Modify: `src/lib.rs`
- Modify: `src/bin/terminai.rs`
- Test: `src/tests/e2e/test_ai.rs`

**Steps:**

1. Add failing UI tests for the control panel rows, dangerous auto-approval confirmation, clear-history confirmation, selection movement, and default confirm/cancel focus.
2. Implement the minimal control UI renderer and state enum for panel, agent picker, auto-approval confirmation, clear-history confirmation, and agent-switch confirmation.
3. Route F7–F10 through the new configurable bindings only after `ai_visible` is true and before input is forwarded to the agent PTY.
4. Make F7 disable auto-approval immediately or open its confirmation before enabling; make F9 confirm and call the VT clear primitive, then reinitialize `ScrollbackTracker`.
5. Wire F10 panel navigation to the same actions and run the focused UI/binary tests.

### Task 5: Confirmed agent switching

**Files:**
- Modify: `src/shell.rs`
- Modify: `src/agent_terminal.rs`
- Modify: `src/agent_launcher.rs`
- Modify: `src/bin/terminai.rs`

**Steps:**

1. Add a focused test for picker inventory: bundled presets are always present, hidden user presets are absent, and a distinct configured startup agent is present.
2. Retain portable-pty's cloned child killer in `Shell` and expose `AgentTerminal::terminate`.
3. Store the active runtime agent selection in `AppState`; use it when rebuilding launch plans after CWD or config changes.
4. Build and validate a selected preset's launch plan before confirmation. On confirmation, drop the old event receiver, terminate and discard the old agent, install the new plan, and launch it with the existing dimensions.
5. Keep failures before confirmation non-destructive; use the existing overlay launch-error display for failures after termination.
6. Run agent launcher, shell, and binary tests.

### Task 6: Documentation, schemas, and regression verification

**Files:**
- Modify: `README.md`
- Modify: `terminai.example.yaml`
- Create: next versioned file under `docs/schema-v*.json`
- Regenerate: `docs/config.html`
- Modify: `CHANGELOG.md`

**Steps:**

1. Document startup approval mode, the danger semantics of auto-approval, overlay-only hotkeys, control-panel behavior, history scope, agent switching, and `show-in-switcher`.
2. Regenerate the configuration schema/reference using the repository's existing Taskfile command and inspect the diff.
3. Run `cargo fmt --all -- --check`, `cargo test -p termin`, and `cargo clippy -p termin --all-targets -- -D warnings` (allowing only documented pre-existing failures if reproduced on the unchanged base).
4. Review `git diff --check` and `git status --short`, ensuring the unrelated `projects/` directory was not touched.
