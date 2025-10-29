# mprocs Patches for Termin.AI

**Base Version:** mprocs v0.7.3
**Repository:** https://github.com/pvolok/mprocs
**Integration Date:** 2025-10-29

---

## Overview

This document tracks all modifications made to the mprocs codebase for Termin.AI integration. The goal is to minimize changes and maintain compatibility with upstream mprocs updates.

**Guiding Principles:**
1. Keep patches minimal (<100 lines total)
2. All changes clearly marked with `// TERMIN.AI:` comments
3. Use additive changes (new variants, new fields) where possible
4. Document merge strategy for each patch

---

## Patch Summary

| File | Lines Changed | Type | Upstream Risk |
|------|---------------|------|---------------|
| `src/config.rs` | ~30 | Additive (new struct) | Low |
| `src/app.rs` | ~20 | Additive (new process) | Low |
| `src/protocol.rs` | ~5 | Additive (enum variants) | Low |
| `src/keymap.rs` | ~10 | Additive (command variant) | Low |
| `src/main.rs` | ~10 | Additive (conditional logic) | Low |
| **TOTAL** | **~75 lines** | | |

---

## Patch 1: AI Configuration Support

**File:** `src/config.rs`
**Lines:** Add after existing structs
**Change Type:** Additive (new struct + field)

```rust
// TERMIN.AI: Start - AI Configuration
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AIConfig {
    pub enabled: bool,
    pub provider: String,
    pub model: String,
    pub activation_key: Option<String>,
    pub providers: Option<std::collections::HashMap<String, ProviderConfig>>,
    pub safety: Option<SafetyConfig>,
    pub privacy: Option<PrivacyConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderConfig {
    pub api_key_env: String,
    pub models: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SafetyConfig {
    pub safe_commands: Vec<String>,
    pub dangerous_commands: Vec<String>,
    pub default_approval: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivacyConfig {
    pub enabled: bool,
    pub redact_patterns: Vec<String>,
}
// TERMIN.AI: End - AI Configuration

// TERMIN.AI: Add to Config struct
pub ai: Option<AIConfig>,
```

**Reason:** Extend configuration to support AI settings
**Upstream Impact:** None (new optional field)
**Merge Strategy:** Copy structs to new versions, field is optional so backward compatible

---

## Patch 2: AI Command Variants

**File:** `src/protocol.rs`
**Lines:** Add to ClientCommand enum
**Change Type:** Additive (enum variants)

```rust
pub enum ClientCommand {
    // ... existing variants ...

    // TERMIN.AI: AI commands
    ToggleAI,
    SendAIMessage { message: String },
    ApproveCommand { command_id: String },
    DenyCommand { command_id: String },
    // TERMIN.AI: End
}
```

**Reason:** Add protocol support for AI interactions
**Upstream Impact:** Low (adds enum variants)
**Merge Strategy:** Add variants to enum in new versions

---

## Patch 3: AI Key Binding

**File:** `src/keymap.rs`
**Lines:** Add to Command enum
**Change Type:** Additive (enum variant + default binding)

```rust
pub enum Command {
    // ... existing variants ...

    // TERMIN.AI: AI commands
    ToggleAI,
    // TERMIN.AI: End
}

// In Keymap::default() or similar:
// TERMIN.AI: Add default AI key binding
keymap.insert(Key::new(KeyCode::Char(' '), KeyModifiers::CONTROL), Command::ToggleAI);
// TERMIN.AI: End
```

**Reason:** Add AI toggle to key command system
**Upstream Impact:** Low (adds enum variant)
**Merge Strategy:** Add variant to enum, add key binding to defaults

---

## Patch 4: AI Process Integration

**File:** `src/app.rs`
**Lines:** Add in create_app_proc or similar
**Change Type:** Additive (conditional initialization)

```rust
// TERMIN.AI: Start - Initialize AI process if enabled
use crate::ai_proc::AIChatProcess;

// In create_app_proc or appropriate init function:
if let Some(ai_config) = &config.ai {
    if ai_config.enabled {
        let ai_proc = AIChatProcess::new(ai_config.clone());
        // Register AI process with kernel
        // (specific implementation depends on mprocs process registration)
    }
}
// TERMIN.AI: End
```

**Reason:** Initialize AI process when enabled
**Upstream Impact:** None (conditional, only runs if AI config present)
**Merge Strategy:** Copy conditional block to appropriate location

---

## Patch 5: Conditional Module Imports

**File:** `src/main.rs`
**Lines:** Add module declarations
**Change Type:** Additive (module declarations)

```rust
// TERMIN.AI: Start - AI modules
#[cfg(feature = "ai")]
mod llm;
#[cfg(feature = "ai")]
mod ai_proc;
#[cfg(feature = "ai")]
mod command;
#[cfg(feature = "ai")]
mod privacy;
// TERMIN.AI: End
```

**Reason:** Include AI modules in build
**Upstream Impact:** None (behind feature flag)
**Merge Strategy:** Keep module declarations at top of file

---

## Feature Flag (Optional)

To make AI features completely optional:

**File:** `Cargo.toml`
**Addition:**

```toml
[features]
default = ["ai"]
ai = ["genai", "reqwest"]

[dependencies]
# Existing dependencies...

# AI dependencies (optional)
genai = { version = "0.3", optional = true }
reqwest = { version = "0.11", features = ["json", "stream", "rustls-tls"], optional = true }
```

**Benefit:** Can build without AI features: `cargo build --no-default-features`

---

## Merge Workflow

### When Updating from Upstream

1. **Fetch upstream changes:**
   ```bash
   git remote add mprocs https://github.com/pvolok/mprocs.git
   git fetch mprocs
   ```

2. **Check for conflicts in patched files:**
   ```bash
   git diff mprocs/main -- src/config.rs src/app.rs src/protocol.rs src/keymap.rs src/main.rs
   ```

3. **For each patched file:**
   - Review upstream changes
   - Reapply patches marked with `// TERMIN.AI:`
   - Test build: `cargo build`
   - Test functionality: `cargo run`

4. **Update this document:**
   - Note new mprocs version
   - Update line numbers if needed
   - Document any new challenges

---

## Testing After Merge

### Verification Checklist

- [ ] `cargo build` succeeds
- [ ] `cargo test` passes
- [ ] mprocs runs without AI (original functionality)
- [ ] mprocs runs with AI enabled
- [ ] AI activation key works
- [ ] AI chat interface renders
- [ ] LLM responses stream correctly
- [ ] Command parsing works
- [ ] Command execution with approval works
- [ ] All mprocs features still function

---

## Contribution Strategy

### Patches to Submit Upstream

Some changes may be valuable to main mprocs:

1. **Generic Process API improvements** - If we make process registration more flexible
2. **Configuration extensibility** - If we add helpful config utilities
3. **Bug fixes** - Any bugs discovered while integrating

### Collaboration

- Open issues for bugs found in mprocs
- Submit PRs for generally useful improvements
- Discuss AI integration approach with mprocs maintainer
- Consider eventual AI as optional mprocs plugin

---

## Version History

### v0.7.3 (Base)
- **Date:** 2025-10-29
- **Status:** Initial integration
- **Patches:** 5 patches, ~75 lines total
- **Notes:** Clean integration, no conflicts

### [Future versions will be documented here]

---

## Notes

### Why So Few Changes?

mprocs architecture is **exceptionally well-designed** for extension:

1. **Process-based architecture** - Easy to add new process types
2. **Message passing** - Clean communication between components
3. **Flexible configuration** - YAML system easily extended
4. **Protocol-based remote control** - Simple to add new commands
5. **Modular structure** - Clear separation of concerns

### Maintenance Burden

**Low:** All AI code is isolated in new modules. Updating mprocs requires only:
1. Checking 5 files for conflicts
2. Re-applying clearly marked patches
3. Testing (mostly automated)

**Estimated time per upstream sync:** 1-2 hours

---

## Contact

For questions about these patches:
- Review `IMPLEMENTATION_PLAN.md` for architecture
- Check `src/llm/` and `src/ai_proc/` for AI-specific code
- Refer to original mprocs docs for core functionality

---

**Last Updated:** 2025-10-29
**mprocs Version:** v0.7.3
**Termin.AI Version:** v0.1.0-dev
