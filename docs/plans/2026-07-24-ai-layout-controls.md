# AI Layout Controls Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add configurable AI height, guest display modes, fullscreen, separator-only decoration, and consolidated layout controls.

**Architecture:** Extend existing interface/runtime state with three small layout values and make the current geometry helpers authoritative. Reuse the current control modal and event routing for settings and layout mode.

**Tech Stack:** Rust, Serde, Crokey, Ratatui, Cargo tests.

---

### Task 1: Configuration

**Files:**
- Modify: `src/terminai_config.rs`
- Modify: `src/terminai_config_init.rs`

1. Add `fullscreen`, `GuestDisplayMode`, 50% height, and `F9`/`F10`/`F11`
   layout/control bindings.
2. Remove the old direct management bindings.
3. Add deserialization/default tests.
4. Run `cargo test -p termin terminai_config`.

### Task 2: Shared layout geometry

**Files:**
- Modify: `src/bin/terminai.rs`

1. Add focused tests for 20–80% sizing, fullscreen, top/bottom areas, and the
   three guest modes.
2. Update the existing overlay/inner/guest geometry helpers.
3. Route agent and guest PTY resize through those helpers.
4. Run the focused binary tests.

### Task 3: Controls and rendering

**Files:**
- Modify: `src/ui_controls.rs`
- Modify: `src/bin/terminai.rs`

1. Extend Terminai Controls with fullscreen and Layout Mode.
2. Add selectable layout-mode rows and direct layout keys.
3. Replace the full AI border with the adjoining separator and arrow title;
   omit decoration in fullscreen and overlay AUTO status at top right.
4. Add focused rendering/navigation tests and run them.

### Task 4: Documentation and verification

**Files:**
- Modify: `README.md`
- Modify: `terminai.example.yaml`

1. Document configuration, semantics, and shortcuts.
2. Run `cargo fmt --check`.
3. Run `cargo test -p termin`.
