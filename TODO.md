# Termin.AI TODO List

**Last Updated:** 2025-12-29
**Status:** Post-LLM Migration Cleanup

---

## High Priority

### AI Integration

#### 1. Command Suggestion Modal
**Location:** `src/bin/terminai.rs:883`
**Issue:** When AI suggests commands, we need an async task to check suggestions and show approval modal
**Context:**
```rust
// TODO: Spawn async task to check suggestions and show modal
```
**Action Items:**
- [ ] Design approval modal UI
- [ ] Implement async task to poll for command suggestions
- [ ] Add approval/rejection handlers
- [ ] Integrate with command executor

#### 2. Event Loop Architecture
**Location:** `src/bin/terminai.rs:434`
**Issue:** Need proper PollCrossterm and PollShell implementation for Phase 2+
**Context:**
```rust
// TODO: Phase 2+: Use PollCrossterm and PollShell properly
```
**Action Items:**
- [ ] Review current event polling implementation
- [ ] Design proper event loop architecture
- [ ] Implement PollCrossterm for terminal events
- [ ] Implement PollShell for shell process events
- [ ] Integrate with AI overlay timing requirements

**Related:** See IMPLEMENTATION_PLAN.md for phased rollout

---

## Medium Priority

### Error Handling

#### 3. Error Display in UI
**Location:** `src/app.rs:242`
**Issue:** Errors are silently swallowed, need user-visible error messages
**Context:**
```rust
// TODO: Show error.
```
**Action Items:**
- [ ] Design error notification system (toast? status bar?)
- [ ] Implement error message display
- [ ] Add error recovery UI
- [ ] Test error scenarios

---

### Feature Completeness

#### 4. Client-Server Mode (Disabled)
**Location:** `src/app.rs:583`
**Issue:** Client-server mode disabled for mprocs 0.7
**Context:**
```rust
// TODO: Client-server mode is disabled for mprocs 0.7
```
**Decision Needed:**
- Do we need client-server mode for Termin.AI?
- Original mprocs feature, may not align with our single-shell model
**Action Items:**
- [ ] Evaluate if client-server mode is needed
- [ ] If yes: Re-implement for Termin.AI architecture
- [ ] If no: Remove TODO and related code

#### 5. Process Duplication Support
**Location:** `src/app.rs:759`
**Issue:** Copy dependencies for duplicate processes not implemented
**Context:**
```rust
log::error!("TODO: Copy deps for duplicate proc.");
```
**Decision Needed:**
- Do we support process duplication in Termin.AI?
- This is an mprocs feature we may not need
**Action Items:**
- [ ] Evaluate if process duplication is needed
- [ ] If yes: Implement dependency copying
- [ ] If no: Remove TODO and related code

---

## Low Priority (Refinements)

### Terminal Emulation

#### 6. Application Keypad Mode
**Location:** `src/encode_term.rs:65`
**Issue:** Need to respect application_keypad mode
**Action Items:**
- [ ] Review VT100 application keypad spec
- [ ] Implement keypad mode handling
- [ ] Test with apps that use keypad mode (vim, emacs)

#### 7. Incomplete Key Mappings
**Locations:**
- `src/encode_term.rs:586` (TODO)
- `src/encode_term.rs:592` (TODO)
- `src/encode_term.rs:596` (TODO)
- `src/key.rs:134` (TODO)
- `src/key.rs:138` (TODO)
**Issue:** Some key codes not yet mapped
**Action Items:**
- [ ] Audit unmapped key codes
- [ ] Implement remaining mappings
- [ ] Test with various terminal apps
- [ ] Document any intentionally unmapped keys

#### 8. CSI Sequence Handling
**Location:** `src/term/input_parser.rs:232`
**Issue:** Incomplete CSI (Control Sequence Introducer) handling
**Action Items:**
- [ ] Review incomplete CSI cases
- [ ] Implement missing CSI handlers
- [ ] Add error recovery for malformed sequences

---

## Technical Debt

#### 9. Widget Rendering Optimization
**Location:** `src/widgets/text_input.rs:39`
**Issue:** Should render directly instead of current approach
**Action Items:**
- [ ] Profile current rendering performance
- [ ] Implement direct rendering
- [ ] Benchmark improvement
- [ ] Ensure no visual regressions

#### 10. Network Framing Optimization
**Location:** `src/host/sender.rs:32`
**Issue:** Should use `framed.feed()` for better performance
**Action Items:**
- [ ] Replace manual framing with `feed()`
- [ ] Test network throughput
- [ ] Verify no protocol regressions

---

## Code Quality Improvements

### Linting

#### 11. Address `#[allow(...)]` Directives
**Status:** Catalogued but not addressed in cleanup
**Locations:**
- `src/ai_proc/mod.rs`: 3× unused_imports (public API re-exports)
- `src/command/mod.rs`: 1× unused_imports (CommandExecutor IS used, can remove)
- `src/clipboard.rs`: 2× dead_code
- `src/host/socket.rs`: 3× (2× unused_imports, 1× dead_code)
- `src/proc/mod.rs`: 1× large_enum_variant (consider Boxing)

**Action Items:**
- [ ] Remove #[allow(unused_imports)] from command/mod.rs (false positive)
- [ ] Box large enum variants in proc/mod.rs
- [ ] Audit and remove dead_code or add justification comments
- [ ] Move unused_imports to #[cfg(...)] blocks where appropriate

---

## Documentation Needs

### Architecture

#### 12. LLM Architecture Documentation
**Status:** Identified in cleanup plan but not created
**Action Items:**
- [ ] Document Rust ↔ Python split rationale
- [ ] Explain AG-UI protocol integration
- [ ] Document client-side vs server-side tools
- [ ] Diagram tool execution flow
- [ ] Document Provider enum duplication (Rust vs Python)

**Output:** `docs/llm_architecture.md`

#### 13. Provider Duplication Documentation
**Action Items:**
- [ ] Add doc comments to `src/llm/providers.rs` explaining:
  - Why duplicated with Python
  - Rust: Pre-flight validation
  - Python: Actual provider implementation
  - Keep in sync when adding providers

---

## Future Enhancements

### Python Tools

#### 14. E2E Tests for Python Tools
**Status:** Tools registered but no E2E tests with actual LLM
**Action Items:**
- [ ] Add E2E test using mock LLM that calls read_file_tool
- [ ] Add E2E test using mock LLM that calls grep_files_tool
- [ ] Verify tool results are correctly returned to LLM
- [ ] Test error handling when tools fail

### Safety & Privacy

#### 15. Command Safety Classification
**Status:** Validator exists but needs tuning
**Action Items:**
- [ ] Review safe command list
- [ ] Add more command patterns
- [ ] Test with real-world command sequences
- [ ] Document safety classification rules

#### 16. Privacy Filter Patterns
**Status:** Basic patterns implemented
**Action Items:**
- [ ] Add more secret patterns (SSH keys, tokens, etc.)
- [ ] Test false positive rate
- [ ] Document redaction rules
- [ ] Allow user customization

---

## Cleanup Complete ✅

The following items from the cleanup action plan have been completed:

- ✅ Delete deprecated `llm_old/` module (862 LOC removed)
- ✅ Move Provider enum to `src/llm/providers.rs`
- ✅ Register Python tools (read_file, grep_files) with Pydantic AI agent
- ✅ Remove unused integration_example module (220 LOC removed)
- ✅ Verify TerminalContext consolidation (already complete)
- ✅ All tests passing (58 Rust, 18 Python)

**Total Cleanup Impact:** ~1,082 LOC removed, +68 LOC added for tool registration

---

## Notes

### Decision Framework

When evaluating TODOs:
1. **Does it align with ORIGINAL_PRD.md?** - If not in scope, remove
2. **Is it needed for single-shell model?** - mprocs features may not apply
3. **What's the user impact?** - Prioritize user-facing issues
4. **What's the effort?** - Quick wins vs major refactors

### Priority Levels

- **High Priority:** User-facing features, AI integration, core functionality
- **Medium Priority:** Error handling, feature completeness decisions
- **Low Priority:** Refinements, edge cases, minor optimizations
- **Technical Debt:** Code quality, performance optimizations

---

**For questions or updates, refer to:**
- Product Requirements: `ORIGINAL_PRD.md`
- Technical Plan: `IMPLEMENTATION_PLAN.md`
- Development Guide: `CLAUDE.md`
