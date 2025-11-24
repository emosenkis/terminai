# Strategic Pivot: Clean Binary Approach

**Date:** 2025-11-24
**Decision:** Abandon mprocs integration approach, build clean binary

---

## Problem Statement

Initial attempt to integrate AI into mprocs resulted in unwanted UI elements:
- ❌ Process list panes (left sidebar)
- ❌ Help bar at bottom
- ❌ Window borders around terminal
- ❌ Multi-process management UI

**Root Cause:** mprocs is a complete application with its own UI/UX. Trying to extend it meant inheriting its entire UI framework.

---

## Solution: Separate Binary

### New Architecture

**Two Binaries:**
1. `termin` (main.rs) - Original mprocs, unchanged, for reference
2. `terminai` (bin/terminai.rs) - **Clean termin.ai implementation**

### What terminai Does

**Uses from mprocs (as library):**
- `vt100/` - Terminal emulation (VT100 parsing, screen rendering)
- `proc/` - PTY management (spawn shell, I/O handling)
- `term/` - Terminal abstractions
- Basic types (`key.rs`, `event.rs`)

**Does NOT use from mprocs:**
- ❌ Application structure (`app.rs`, `kernel/`)
- ❌ Config system (`config.rs`, `settings.rs`)
- ❌ UI components (`ui_*.rs`, `modal/`)
- ❌ Multi-process logic

**Adds from termin.ai:**
- ✅ AI modules (LLM client, command parsing, safety, privacy)
- ✅ Minimal app structure (just shell + AI overlay)
- ✅ Clean keyboard handling (no complex keymap system)

---

## Implementation Details

### File Structure

```
src/
├── lib.rs                  # NEW: Exports modules for binaries
├── main.rs                 # UNCHANGED: Original mprocs binary
├── bin/
│   └── terminai.rs         # NEW: Clean termin.ai binary
├── llm/                    # Termin.AI modules (ready to use)
├── ai_proc/
├── command/
├── privacy/
├── vt100/                  # mprocs modules (used as library)
├── proc/
├── term/
└── ...                     # Other mprocs modules (mostly unused)
```

### Binary Behavior

**`termin` (mprocs):**
```bash
$ cargo build --bin termin
$ ./target/debug/termin
# Shows mprocs UI with process list, help bar, etc.
# Used for reference/comparison only
```

**`terminai` (termin.ai):**
```bash
$ cargo build --bin terminai
$ ./target/debug/terminai
# Clean shell wrapper, no UI chrome
# Ctrl-Space: AI overlay (works even without API key)
# ESC: close overlay
# Ctrl-C: quit
```

---

## Key Decisions

### 1. AI Overlay Always Available

**Decision:** Show AI overlay even when ANTHROPIC_API_KEY is not set

**Rationale:**
- Users can see the interface
- Clear instructions on how to configure
- Better onboarding experience

**Implementation:**
```rust
// Ctrl-Space always toggles overlay
self.ai_visible = !self.ai_visible;

// If no AI key, overlay shows:
// "AI not configured. Set ANTHROPIC_API_KEY to enable."
```

### 2. Revert mprocs Binary

**Decision:** Keep `termin` binary as original mprocs

**Rationale:**
- Don't need to modify mprocs anymore
- Keep it as reference implementation
- Users who want mprocs get the real thing
- termin.ai is `terminai` binary

### 3. Library Exports

**Decision:** Create `lib.rs` to export mprocs modules

**Rationale:**
- Allows `terminai` binary to import from same crate
- Clean module boundaries
- Easy to see what we're using

---

## Migration Path

### What Changed

**PROGRESS.md:**
- Updated to reflect clean binary approach
- Revised phases to focus on terminai
- Clear distinction between mprocs (library) and termin.ai (product)

**main.rs:**
- ✅ **REVERTED** to upstream mprocs
- No longer used for termin.ai

**bin/terminai.rs:**
- ✅ **NEW** clean implementation
- Minimal skeleton (150 LOC)
- Ready for PTY integration

**lib.rs:**
- ✅ **NEW** module exports
- Makes crate usable as library

### What Stayed Same

**AI Modules:**
- ✅ All AI functionality intact
- ✅ All 34 tests still passing
- ✅ Ready to integrate into terminai

**mprocs Foundation:**
- ✅ PTY handling code unchanged
- ✅ VT100 emulation unchanged
- ✅ Can be used as library

---

## Next Steps

### Immediate (This Week)

1. **Shell PTY Integration** (2-4 hours)
   - Spawn shell via portable-pty
   - Read PTY output
   - Parse with VT100
   - Render to screen (full screen, no borders)

2. **AI Overlay Rendering** (1-2 hours)
   - Render AIChatUI when ai_visible = true
   - Show "not configured" message if no API key
   - Handle keyboard input in overlay

3. **Input Routing** (1 hour)
   - Passthrough keys to shell when overlay hidden
   - Route keys to AI when overlay visible
   - ESC to close overlay

### Near Term (Next Week)

4. **Context Extraction**
   - Capture shell output via VT100
   - Extract for AI context

5. **LLM Integration**
   - Send messages to LLM
   - Stream responses
   - Display in overlay

6. **Command Execution**
   - Parse commands from AI responses
   - Safety validation
   - Approval workflow
   - Inject into PTY

---

## Success Metrics

### MVP Criteria (v0.1.0)

| Requirement | Target | Status |
|-------------|--------|--------|
| Binary size | <10MB | TBD |
| Startup time | <100ms | TBD |
| No UI chrome | 100% | ✅ Architecture complete |
| Single shell | Yes | ✅ Planned |
| AI overlay | Ctrl-Space | ✅ Skeleton ready |
| Works without API key | Yes | ✅ Implemented |
| Command approval | Yes | ✅ Module ready |
| Context-aware | Yes | ✅ Module ready |

---

## Risks Mitigated

| Risk | Old Approach | New Approach |
|------|--------------|--------------|
| Unwanted UI elements | High - inherited from mprocs | **Eliminated** - clean binary |
| Complex integration | High - fighting mprocs architecture | **Low** - use as library only |
| Breaking changes | High - modifying mprocs core | **None** - separate binary |
| Maintenance burden | High - track mprocs changes | **Low** - stable PTY/VT100 API |

---

## Team Communication

### For Documentation Updates

- ✅ PROGRESS.md - Updated with clean binary approach
- ⏳ IMPLEMENTATION_PLAN.md - Needs update (Phase 0 focus)
- ✅ CLAUDE.md - Still accurate (principles apply)
- ✅ MPROCS_BORROWED.md - Still accurate (library usage)

### For Users

**README.md should clarify:**
- Two binaries: `termin` (mprocs) and `terminai` (termin.ai)
- `terminai` is the termin.ai product
- `termin` is original mprocs (kept for reference)

### For Developers

**Key takeaway:** Don't think of this as "modifying mprocs". Think of it as:
- **Using mprocs' PTY/VT100 code as a library**
- **Building a new app (terminai) on top of that library**

---

## Conclusion

This pivot resolves the fundamental mismatch between:
- **mprocs** = Multi-process manager with rich UI
- **termin.ai** = Transparent shell wrapper with AI overlay

By treating mprocs as a library (not a base to extend), we get:
- ✅ Clean separation of concerns
- ✅ No unwanted UI inheritance
- ✅ Freedom to build exactly what the PRD specifies
- ✅ Easier maintenance (minimal mprocs coupling)

**Status:** Architecture validated, ready to implement core functionality.

---

**Last Updated:** 2025-11-24
**Next Review:** After Phase 0 completion (shell rendering working)
