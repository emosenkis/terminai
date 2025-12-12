# Termin.AI rat-salsa Migration Status

**Date:** 2025-12-12
**Current Phase:** Phase 1 (In Progress)

## Summary

The migration to rat-salsa architecture is underway but requires more extensive changes than initially estimated due to fundamental architectural differences between the custom event loop and rat-salsa's framework.

## Completed Work

### Phase 1 Progress (60% complete)

✅ **Task 1: Add rat-salsa dependencies**
- Added rat-salsa, rat-focus, rat-event, rat-widget, rat-scrolled, rat-theme4
- Resolved chrono::Locale conflicts by disabling unused modules
- Dependencies build successfully

✅ **Task 2a: Create basic structures**
- Created `Global` struct implementing `SalsaContext`
- Created `AppEvent` enum with shell event variants
- Renamed `App` to `AppState`, removed `terminal` field (managed by rat-salsa)

✅ **Task 3: Implement PollShell**
- Created `PollShell` struct implementing `PollEvents` trait
- Implements proper poll/read pattern with event caching
- Handles shell output, terminal replies, and exit events

✅ **Task 4a: Extract initialization logic**
- Created `initialize_app_components()` async helper
- Created `initialize_ai()` async helper
- Moved AI initialization out of App::new

## Remaining Work

### Phase 1 (40% remaining)

❌ **Task 2b: Implement rat-salsa functions**
- Need to create `init()` function
- Need to create `render()` function
- Need to create `event()` function
- Need to create `error()` function

❌ **Task 4b: Migrate shell rendering**
- Need to adapt `App::render()` to work with rat-salsa's render function
- Need to handle scrollback rendering in rat-salsa context
- Need to remove old `App::run()` event loop

❌ **Task 4c: Wire up event sources**
- Need to extract `shell.event_rx` before moving shell to AppState
- Need to pass `PollShell` to `RunConfig`
- Need to enable `run_tui()` call in main()

❌ **Task 5: Testing**
- Shell launches correctly
- Keyboard input routes to shell
- Terminal resizing works
- Shell exit closes application
- Build, lint, test pass

### Phases 2-6 (Not Started)

All subsequent phases depend on Phase 1 completion.

## Technical Challenges Encountered

### 1. Async/Sync Impedance Mismatch

**Problem:** The original code uses async/await extensively (esp. for AI initialization), but rat-salsa's `run_tui()` is synchronous.

**Solution Applied:** Moved async initialization (`initialize_app_components()`) before calling `run_tui()`.

**Remaining:** Need to handle async AI message sending (will use `ctx.spawn_async()`).

### 2. Event Loop Architecture Difference

**Problem:** Original code uses `tokio::select!` with custom event handling. rat-salsa uses a poll-based system with `PollEvents` trait.

**Solution Applied:** Implemented `PollShell` with proper poll/read separation and event caching.

**Remaining:** Need to integrate shell event handling into rat-salsa event() function.

### 3. Terminal Ownership

**Problem:** Original App struct owned the Terminal, but rat-salsa manages the terminal internally.

**Solution Applied:** Removed terminal field from AppState.

**Remaining:** Need to adapt rendering code to use rat-salsa's terminal.draw() pattern.

### 4. Shell Event Receiver Extraction

**Problem:** `Shell` struct owns `event_rx`, but `PollShell` needs it. Can't move it out after Shell is moved into AppState.

**Options:**
- A) Modify Shell to return event_rx separately (requires changing shell module)
- B) Use Arc/Mutex wrapping (adds overhead)
- C) Add Shell::take_event_rx() method

**Status:** Not yet resolved. Need to choose approach.

## Current Code State

**Status:** DOES NOT COMPILE

**Errors:**
- Old `App::new()` and `App::run()` methods still exist and reference removed `terminal` field
- Missing `init()`, `render()`, `event()`, `error()` functions

**Last Working Commit:** `48ec18ab` - "Phase 1: Add rat-salsa dependencies and resolve conflicts"

**Current WIP Commit:** `2a5642cb` - "WIP: Migrate terminai.rs to rat-salsa architecture (incomplete)"

## Estimated Effort to Complete

### Phase 1 Completion
- **Remaining effort:** 4-6 hours
- **Complexity:** High
  - Need to understand rat-salsa's terminal handling
  - Need to port scrollback rendering logic
  - Need to solve shell event_rx extraction issue
  - Need to integrate AI overlay rendering (may defer to Phase 2)

### Phases 2-6
- **Total estimated effort:** 15-20 hours (as per SALSA.md)
- **Dependencies:** Blocked on Phase 1 completion

## Recommended Next Steps

### Option A: Complete Minimal Phase 1
1. Create minimal `init()`/`render()`/`event()`/`error()` functions (shell only, no AI yet)
2. Solve shell event_rx extraction
3. Get basic shell terminal working with rat-salsa
4. Commit working version
5. Add AI features back in Phase 2

### Option B: Complete Full Migration
1. Port all existing functionality (shell + AI) to rat-salsa in one go
2. More complex but avoids incremental complications
3. Longer before first working commit

### Option C: Pause and Reassess
1. Evaluate if rat-salsa is the right choice given complexity
2. Consider alternative approaches (e.g., keep custom event loop, just add focus management)
3. Discuss scope/timeline with stakeholder

## Decision

**Proceeding with Option A** - Incremental approach with minimal working versions at each step.

---

**Author:** Claude (AI Assistant)
**Last Updated:** 2025-12-12
