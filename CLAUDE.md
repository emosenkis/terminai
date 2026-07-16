# Instructions for Claude Code When Working on Terminai

**Last Updated:** 2025-11-14
**Project:** Terminai - Interactive Terminal with AI Assistant
**Status:** Active Development (v0.1.0)

---

## Plan File Location

**All new design documents, implementation plans, and other planning artifacts must be created under `./plans/`. Never put plans under `./docs/`: that directory is the public website published at <https://terminai.app/>.**

When a workflow or skill suggests `docs/plans/...`, use `plans/...` instead.

---

## Overview

You are working on **Terminai**, a terminal wrapper that provides AI assistance through an overlay interface. This is a Rust project that borrows terminal virtualization code from mprocs but builds a completely different product.

**Critical:** This is NOT an mprocs extension. It's a single-shell terminal with AI overlay.

---

## Required Reading (In Order)

### 1. FIRST: Understand the Product Vision

**Read:** `ORIGINAL_PRD.md`
- **Purpose:** The definitive product requirements document
- **Authority:** This is the source of truth for what we're building
- **Focus:** User experience, features, success criteria
- **Key Sections:** Core Features, User Stories, Non-Functional Requirements

**What You'll Learn:**
- Transparent shell wrapper operation
- AI overlay activation (Ctrl-Space)
- Context-aware chat interface
- Command execution with approval
- Safety and privacy requirements

### 2. SECOND: Understand the Technical Strategy

**Read:** `IMPLEMENTATION_PLAN.md`
- **Purpose:** Technical architecture and development roadmap
- **Authority:** The source of truth for how we're building it
- **Focus:** Architecture, modules, phases, code structure
- **Key Sections:** Architecture (single shell + overlay), Module Additions, Development Phases

**What You'll Learn:**
- We borrow mprocs' VT100/PTY code (~30%)
- We build our own app/UI/AI layers (~70%)
- Single shell focus (not multi-process)
- 5-phase development plan
- Module structure and responsibilities

### 3. THIRD: Understand the mprocs Relationship

**Read:** `MPROCS_BORROWED.md`
- **Purpose:** Documents what code we borrowed from mprocs
- **Authority:** Defines our relationship with upstream mprocs
- **Focus:** What we reuse vs replace, update strategy
- **Key Sections:** Modules Borrowed, Modules Replaced, Cherry-Picking Strategy

**What You'll Learn:**
- Which mprocs modules to keep (vt100/, proc/, term/)
- Which modules to replace (app.rs, config.rs, ui_*.rs)
- How to handle mprocs updates
- We're NOT maintaining fork compatibility

### 4. OPTIONAL: Integration History

**Read:** `INTEGRATION_SUMMARY.md`
- **Purpose:** Historical context on strategic decisions
- **When to Read:** If you need background on why we chose this approach
- **Focus:** Benefits, timeline, comparisons

---

## Working Principles

### 1. Follow the PRD Strictly

**The PRD (`ORIGINAL_PRD.md`) is the contract with the user.**

When implementing features:
- ✅ Implement exactly what the PRD specifies
- ✅ Match the user experience described in User Stories
- ✅ Meet the success criteria defined
- ✅ Respect the constraints and non-functional requirements

**If you discover issues with the PRD:**
- 🚨 STOP and raise the issue
- Explain the problem clearly
- Suggest specific changes to the PRD
- Wait for user approval before deviating

### 2. Follow the Implementation Plan Strictly

**The Implementation Plan (`IMPLEMENTATION_PLAN.md`) is the technical roadmap.**

When writing code:
- ✅ Follow the architecture diagrams
- ✅ Place code in the specified modules
- ✅ Implement phases in order
- ✅ Use the defined interfaces and types
- ✅ Follow the file structure

**If you discover issues with the plan:**
- 🚨 STOP and raise the issue
- Explain the technical problem
- Suggest specific changes to the plan
- Provide alternative approaches if possible
- Wait for user approval before deviating

### 3. Preserve Product Identity

**Terminai is NOT mprocs.**

Never:
- ❌ Add multi-process management features
- ❌ Add config-driven process launching
- ❌ Build a process list UI
- ❌ Try to maintain mprocs compatibility
- ❌ Preserve mprocs' application structure

Always:
- ✅ Focus on single-shell experience
- ✅ Build AI overlay as core feature
- ✅ Transparent operation until AI invoked
- ✅ Command injection into single shell
- ✅ Freedom to redesign for our use case

---

## Issue Detection and Reporting

### When to Raise Issues

You should STOP and raise an issue when you discover:

**1. PRD Ambiguities:**
- Feature descriptions that could be interpreted multiple ways
- Missing user interaction details
- Unclear success criteria
- Conflicting requirements

**Example:**
```
🚨 PRD ISSUE DETECTED

Location: ORIGINAL_PRD.md, "Command Execution" section
Issue: The PRD says "auto-approve safe commands" but doesn't specify
       what happens if a safe command is in a pipe with a dangerous one.
       Example: `cat file.txt | rm`

Question: Should we:
A) Analyze the entire pipeline and require approval if ANY part is dangerous?
B) Only look at the primary command?
C) Block pipelines entirely until user approves?

Suggested PRD Change: Add section "5.5 Command Pipeline Handling" with
specific rules for how to handle chained/piped commands.
```

**2. Implementation Plan Problems:**
- Architecture that won't support PRD requirements
- Module dependencies that create circular references
- Performance concerns with proposed approach
- Missing critical components
- Phase ordering issues

**Example:**
```
🚨 IMPLEMENTATION PLAN ISSUE DETECTED

Location: IMPLEMENTATION_PLAN.md, Phase 2 "AI Assistant Module"
Issue: The plan shows AIAssistant managing conversation history, but
       the PRD requires "Session History and Persistence" (section 8)
       across terminal restarts. Current plan doesn't show where/how
       history is persisted to disk.

Impact: High - Users will lose conversation history on restart

Suggested Plan Changes:
1. Add src/history/ module in Module Additions section
2. Add Phase 2.5 "History Persistence" between current Phase 2 and 3
3. Define history file format (JSON/SQLite/other)
4. Specify AI assistant should load history on startup

Alternative: Add history persistence to Phase 7 "Polish & Testing" if
not critical for MVP.
```

**3. Security/Safety Concerns:**
- Command injection vulnerabilities
- Insufficient validation
- Privacy leaks
- API key exposure

**Example:**
```
🚨 SECURITY ISSUE DETECTED

Location: Implementation plan for command injection
Issue: Current plan suggests injecting commands by writing to PTY.
       If we don't properly escape shell metacharacters, this could
       allow command injection attacks.

Risk: High - AI could accidentally or maliciously inject harmful commands

Example Attack: If AI suggests `ls; rm -rf /` and we inject without
validation, the dangerous part executes.

Suggested Changes:
1. Add shell escaping/quoting to command injector
2. Add command validator BEFORE injection (not just before approval)
3. Consider using shell's proper command API instead of PTY injection
4. Add comprehensive tests for shell metacharacter handling

PRD Update: Add security requirement to section on Command Execution
```

**4. Missing Requirements:**
- Features needed but not specified
- Edge cases not handled
- Error conditions not defined

**Example:**
```
🚨 MISSING REQUIREMENT DETECTED

Issue: PRD and plan don't specify what happens when shell exits
       (user types 'exit' or shell crashes)

Questions:
- Should Terminai exit immediately?
- Offer to restart shell?
- Show AI overlay to ask user what to do?
- Preserve AI conversation history?

This is a critical user experience moment not covered in PRD.

Suggested PRD Addition: Section "9. Shell Lifecycle Management"
- Behavior on normal exit (user types 'exit')
- Behavior on shell crash
- Auto-restart options
- History preservation requirements
```

**5. Technical Impossibilities:**
- Requirements that conflict with platform limitations
- Performance targets that can't be met
- API limitations

**Example:**
```
🚨 TECHNICAL LIMITATION DETECTED

Location: ORIGINAL_PRD.md, "Must work even when applications are running"
Issue: PRD says Ctrl-Space should activate AI "even when applications
       are running." This may not be possible when apps take over terminal
       (e.g., vim, htop) because they intercept all key input.

Platform Limitation: When an app runs in raw mode, it receives ALL
keyboard input before our wrapper can intercept it.

Suggested PRD Changes:
1. Clarify: "Works during idle shell and simple command execution"
2. Document limitation: "May not work in full-screen apps (vim, htop)
   until app exits or user switches to wrapper mode"
3. Add alternative: "Provide fallback activation (maybe ESC key sequence)"

Alternative Solutions to Explore:
- Could we inject a key binding into the running app?
- Could we use a different activation method (e.g., process signal)?
```

---

## How to Raise Issues

### Issue Report Template

```markdown
## 🚨 [ISSUE TYPE] DETECTED

**Document:** [PRD/IMPLEMENTATION_PLAN/other]
**Section:** [specific section or line numbers]
**Severity:** [Critical/High/Medium/Low]

### Problem Description
[Clear explanation of the issue]

### Impact
[How this affects the project/users]

### Questions to Resolve
1. [Question 1]
2. [Question 2]

### Suggested Changes

**Option A: [Approach name]**
- Change: [specific change to PRD/plan]
- Pros: [benefits]
- Cons: [drawbacks]

**Option B: [Alternative approach]**
- Change: [specific change to PRD/plan]
- Pros: [benefits]
- Cons: [drawbacks]

### Recommended Approach
[Your recommendation with reasoning]

### Blocking?
[Yes/No - should work stop until this is resolved?]
```

### Examples of Good Issue Reports

See the examples above in "When to Raise Issues" section.

---

## Development Workflow

### Starting a New Feature

1. **Read relevant PRD section** for the feature
2. **Read relevant Implementation Plan section** for technical approach
3. **Check if borrowed mprocs code is involved** (MPROCS_BORROWED.md)
4. **Verify understanding** with user before starting
5. **Raise any issues** discovered during planning
6. **Implement according to plan** once approved

### During Implementation

1. **Follow the plan strictly** - use specified modules, types, structure
2. **Write tests** as you go (PRD specifies high test coverage)
3. **Raise issues immediately** if you discover problems
4. **Document deviations** if user approves changes
5. **Update plan docs in `./plans/`** if architecture changes

### Before Completing

1. **Verify against PRD** - does it meet the requirements?
2. **Check success criteria** - does it pass acceptance criteria?
3. **Test edge cases** - what could go wrong?
4. **Security review** - any vulnerabilities?
5. **Update documentation** - are plans still accurate?

---

## Code Quality Standards

### From PRD Non-Functional Requirements

**Performance:**
- <100ms startup overhead vs native shell
- <1ms keystroke pass-through latency
- <500ms overlay activation
- <50MB base memory usage

**Security:**
- Never log or display API keys
- Redact sensitive patterns by default
- Secure config file permissions (600)
- Command approval for dangerous operations

**Reliability:**
- Never crash the user's shell session
- Graceful degradation if LLM unavailable
- Auto-recovery from PTY errors
- Comprehensive error messages

### Code Style

**Rust Best Practices:**
- Use `Result` for error handling
- Avoid `unwrap()` except in tests
- Document public APIs
- Use type system for safety
- Async for I/O operations

**Project Conventions:**
- Mark borrowed mprocs code with comments: `// FROM MPROCS: vt100/parser.rs`
- Mark modifications: `// TERMINAI: Modified for single-shell usage`
- Module-level docs explain purpose and responsibilities
- Tests in `tests/` directory or `#[cfg(test)]` modules

---

## Common Pitfalls

### ❌ DON'T: Treat This as mprocs Extension

**Wrong:**
```rust
// Adding AI as another process type to mprocs' multi-process kernel
pub struct AIChatProcess {
    // ... implements mprocs' Process trait
}
```

**Right:**
```rust
// AI assistant that overlays on our single shell
pub struct AIAssistant {
    llm_client: LLMClient,
    conversation: Vec<Message>,
}

pub struct App {
    shell: SingleShellProcess,  // ONE shell
    ai: Option<AIAssistant>,    // Overlay on that shell
}
```

### ❌ DON'T: Preserve mprocs Architecture

**Wrong:**
```rust
// Keeping mprocs' process list UI
pub struct ProcessListPane { ... }
pub struct ProcessTabs { ... }
```

**Right:**
```rust
// Single terminal, full screen
pub struct TerminalView {
    term: Term,  // Full screen terminal (borrowed from mprocs)
}

pub struct AIOverlay {
    // Appears OVER terminal when activated
}
```

### ❌ DON'T: Add Process Management

**Wrong:**
```yaml
# config.toml
procs:
  server:
    shell: "npm run dev"
  tests:
    shell: "npm test"
```

**Right:**
```toml
# config.toml
[ai]
provider = "anthropic"
model = "claude-3-5-sonnet-20241022"

[safety]
safe_commands = ["ls", "pwd"]
```

### ✅ DO: Focus on Single Shell + AI

**Correct approach:**
- Launch user's shell (from $SHELL)
- Full-screen terminal (transparent operation)
- Ctrl-Space: AI overlay appears
- AI can inject commands into that shell
- ESC: back to normal terminal

---

## Module Responsibilities

### Borrowed from mprocs (Minimal Changes)

**`src/vt100/`** - VT100 terminal emulation
- Leave as-is from mprocs
- Only touch for critical bugs
- Cherry-pick mprocs updates

**`src/proc/`** - PTY handling
- Simplify for single process
- Remove multi-process kernel dependencies
- Keep core PTY management

**`src/term/`** - Terminal abstractions
- Use as-is from mprocs
- Minimal modifications

### New Terminai Code

**`src/main.rs`** - Entry point
- Detect user's shell
- Launch single shell
- Initialize app
- Run event loop

**`src/app.rs`** - Application core
- Manage single shell process
- Handle mode switching (Normal/AI/Copy)
- Coordinate shell and AI
- Event handling and rendering

**`src/config.rs`** - Configuration
- Safety/privacy config
- Keybindings
- NO process management config

**`src/ui_shell.rs`** - Terminal UI
- Full-screen terminal display
- Pass-through mode rendering
- Uses borrowed vt100 code

**`src/ui_ai_overlay.rs`** - AI overlay UI
- Chat interface overlay
- Message history display
- Command approval UI
- Input handling

**`src/agent_launcher.rs`** - CLI agent launch plans
- Loads bundled YAML presets
- Expands MCP/context templates
- Resolves user preset overrides

**`src/agent_terminal.rs`** - AI CLI terminal
- PTY-backed agent process
- Terminal rendering and input forwarding

**`src/command/`** - Command handling
- Parse commands from markdown
- Safety classification
- Command injection into PTY
- Approval workflow

**`src/privacy/`** - Privacy filter
- Pattern-based redaction
- Sensitive data detection
- Terminal history sanitization

---

## Testing Requirements

### From PRD

**Unit Tests:**
- 80%+ code coverage
- All command parsing logic
- All safety validation
- Privacy filter patterns

**Integration Tests:**
1. Shell launches correctly
2. AI overlay activates
3. Commands inject into shell
4. Terminal history captured
5. Safety validation works
6. Privacy filter works

**Manual Testing:**
- Cross-platform (Linux, macOS)
- Multiple shells (bash, zsh, fish)
- Various terminal emulators
- AI provider connections
- Command execution workflow

---

## Phase Status Tracking

### Current Phase: [User will update this]

Refer to IMPLEMENTATION_PLAN.md for phase details:
- Phase 0: Restructure for single-shell (Week 1)
- Phase 1: LLM Client (Week 2)
- Phase 2: AI Assistant Module (Week 3)
- Phase 3: Command Injection (Week 4)
- Phase 4: Integration & Polish (Week 5)

### Before Starting Next Phase

1. Review phase requirements in IMPLEMENTATION_PLAN.md
2. Check PRD for related requirements
3. Verify previous phase is complete
4. Raise any issues with phase plan
5. Get user approval to proceed

---

## Quick Reference

### Key Files Priority Order

1. **ORIGINAL_PRD.md** - What we're building (AUTHORITY)
2. **IMPLEMENTATION_PLAN.md** - How we're building it (AUTHORITY)
3. **MPROCS_BORROWED.md** - What code we borrowed
4. **INTEGRATION_SUMMARY.md** - Historical context
5. **MPROCS_PATCHES.md** - DEPRECATED (we're not patching mprocs)

### Decision Authority

**Product Decisions:** PRD is authority
**Technical Decisions:** Implementation Plan is authority
**Code Reuse:** MPROCS_BORROWED.md is authority

### When in Doubt

1. Check PRD first
2. Check Implementation Plan second
3. Raise issue if unclear
4. Ask user for clarification
5. Document decision for future

---

## Communication Protocol

### Reporting Progress

```markdown
## Progress Update

**Feature:** [feature name]
**Phase:** [current phase]
**Status:** [in progress/complete/blocked]

**Completed:**
- [x] Task 1
- [x] Task 2

**In Progress:**
- [ ] Task 3 (60% complete)

**Issues:**
- None / [list issues]

**Next Steps:**
- Task 4
- Task 5
```

### Asking for Clarification

```markdown
## Clarification Needed

**Context:** [what you're working on]
**Question:** [specific question]

**Relevant PRD Section:** [link/quote]
**Relevant Plan Section:** [link/quote]

**Options I'm Considering:**
1. [Option A]: [pros/cons]
2. [Option B]: [pros/cons]

**My Recommendation:** [option and why]

**Blocking:** [Yes/No]
```

---

## Emergency Contacts

**Critical Issues:**
- Security vulnerabilities → STOP immediately, report
- Data loss risks → STOP immediately, report
- PRD conflicts → STOP, request clarification
- Technical impossibilities → STOP, explain limitation

**Non-Blocking Issues:**
- Minor ambiguities → Note and continue, ask when convenient
- Code style questions → Follow Rust conventions
- Performance optimizations → Note for later, focus on correctness first

---

## Final Reminders

1. **READ THE PRD FIRST** - It's the contract
2. **FOLLOW THE PLAN** - Unless you find a problem
3. **RAISE ISSUES EARLY** - Don't wait until you're stuck
4. **SINGLE SHELL FOCUS** - Not multi-process
5. **NOT AN MPROCS FORK** - Different product
6. **SECURITY MATTERS** - Command injection is dangerous
7. **USER EXPERIENCE FIRST** - Transparent until AI needed
8. **TEST EVERYTHING** - Terminal bugs are subtle
9. **DOCUMENT CHANGES** - Update plans if needed
10. **ASK WHEN UNSURE** - Clarity is better than guessing

---

**Remember:** Your job is to build what the PRD specifies, following the Implementation Plan, while actively identifying and reporting issues. You're not just a code generator - you're a quality gate that catches problems before they become bugs.

Good luck! 🚀

---

**Last Updated:** 2025-11-14
**For Questions:** Refer to this file, then ask user
**For Updates:** User will maintain this file
