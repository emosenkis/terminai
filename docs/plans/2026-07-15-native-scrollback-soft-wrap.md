# Native Scrollback Soft-Wrap Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:test-driven-development to implement this plan task-by-task.

**Goal:** Preserve soft-wrapped logical lines when VT100 rows are streamed into host-terminal native scrollback.

**Architecture:** Retain each VT row's `wrapped()` flag in the application snapshot and carry it through Ratatui's `Frame` and `Backend` interfaces. The Crossterm backend will omit `\r\n` only after rows explicitly marked as soft-wrapped, allowing the host terminal to perform its normal automatic wrap.

**Tech Stack:** Rust, Terminai VT100 model, patched Ratatui backend, Crossterm, Cargo tests.

---

### Task 1: Specify backend streaming behavior

**Files:**
- Modify: `ratatui/src/backend/crossterm.rs`

**Step 1: Write failing tests**

Add one test with two full-width rows and wrap flags `[true, false]`; assert that the output contains their text without `\r\n` between them. Add a control test with `[false, false]`; assert that the first row is followed by `\r\n`.

**Step 2: Verify the tests fail**

Run: `cargo test --manifest-path ratatui/Cargo.toml --features native-scrolling stream_lines_to_scrollback_`

Expected: compilation or assertions fail because the backend interface does not yet accept wrap metadata and always emits `\r\n`.

### Task 2: Carry wrap metadata through the streaming path

**Files:**
- Modify: `src/scrollback.rs`
- Modify: `src/bin/terminai.rs`
- Modify: `ratatui/src/backend.rs`
- Modify: `ratatui/src/backend/crossterm.rs`
- Modify: `ratatui/src/backend/test.rs`
- Modify: `ratatui/src/terminal/frame.rs`
- Modify: `ratatui/src/terminal/terminal.rs`
- Modify: `src/tests/e2e/test_resize.rs`
- Modify: `src/tests/e2e/test_scrollback.rs`

**Step 1: Implement the minimal metadata path**

Return a wrap-flag vector from `drain_pending_native_scrollback_snapshot`, store it in `ScrollSnapshot`, and pass it to `Backend::stream_lines_to_scrollback`. Update all backend implementations and forwarding test backends.

**Step 2: Implement the streaming rule**

In the Crossterm backend, emit `\r\n` after a row only when its wrap flag is false. Keep the existing final screen-height padding advances.

**Step 3: Verify focused tests pass**

Run: `cargo test --manifest-path ratatui/Cargo.toml --features native-scrolling stream_lines_to_scrollback_`

Expected: all focused Crossterm streaming tests pass.

**Step 4: Verify application snapshot metadata**

Add a Terminai test that produces a VT soft wrap, drains the snapshot, and asserts the corresponding wrap flag is retained.

Run: `cargo test -p termin test_pending_native_scrollback_snapshot_preserves_soft_wraps`

Expected: pass.

### Task 3: Regression verification

**Files:**
- Modify if needed: files from Tasks 1-2 only

**Step 1: Format changed Rust files**

Run: `cargo fmt --all -- --check`

Expected: pass after formatting.

**Step 2: Run native-scrolling Ratatui tests**

Run: `cargo test --manifest-path ratatui/Cargo.toml --features native-scrolling`

Expected: pass.

**Step 3: Run Terminai tests**

Run: `cargo test -p termin`

Expected: pass.
