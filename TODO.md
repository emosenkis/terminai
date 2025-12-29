# Termin.AI TODO List

**Last Updated:** 2025-12-29
**Status:** Post-LLM Migration Cleanup

---

## High Priority

### AI Integration

#### 1. Command Suggestion Modal ✅ COMPLETED
**Location:** `src/bin/terminai.rs:883`
**Status:** ✅ Implemented
**Implementation:** Async task spawned on ToolExecuted event checks for suggestions via `get_latest_suggestion()` and displays inline approval dialog with emoji risk indicators (🟢🟡🔴)
**Completed:** 2025-12-29
**Action Items:**
- [x] Design approval modal UI - Using inline approval dialog in AI chat
- [x] Implement async task to poll for command suggestions - Spawns on suggest_command tool execution
- [x] Add approval/rejection handlers - Press 'y' to approve, 'n' to reject
- [x] Integrate with command executor - Commands sent to shell via approve_command()

#### 2. Event Loop Architecture ✅ COMPLETED
**Location:** `src/bin/terminai.rs:434`
**Status:** ✅ Implemented
**Implementation:** PollShell properly integrated into rat-salsa framework. Shell events flow through proper event dispatch instead of manual polling.
**Completed:** 2025-12-29
**Action Items:**
- [x] Review current event polling implementation - Manual polling was inefficient
- [x] Design proper event loop architecture - Use rat-salsa PollEvents trait
- [x] Implement PollCrossterm for terminal events - Already registered
- [x] Implement PollShell for shell process events - Registered in RunConfig
- [x] Integrate with AI overlay timing requirements - Event ordering now correct

**Related:** See IMPLEMENTATION_PLAN.md for phased rollout

---

## Medium Priority

### Error Handling

#### 3. Error Display in UI ❌ NOT APPLICABLE
**Location:** `src/app.rs:242`
**Status:** ❌ Not applicable to Termin.AI
**Reason:** This TODO is in the old mprocs app.rs file which handles multi-process management. Termin.AI is single-shell focused and doesn't use process dependencies, so this error case won't occur.
**Note:** Error display is already implemented in the AI chat UI (see `AIChatProcess::set_error()` and error dialog rendering in `src/ai_proc/ui.rs`)

---

## Code Quality Improvements

### Linting

#### 11. Address `#[allow(...)]` Directives
**Status:** Catalogued but not addressed in cleanup
**Locations:**
- `src/ai_proc/mod.rs`: 3× unused_imports (public API re-exports)
- `src/command/mod.rs`: 1× unused_imports (CommandExecutor IS used, can remove)
- `src/host/socket.rs`: 3× (2× unused_imports, 1× dead_code)

**Action Items:**
- [ ] Remove #[allow(unused_imports)] from command/mod.rs (false positive)
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
