# Minijinja Templates Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace Handlebars rendering with Minijinja, add typed expression arguments, and support block-overridable XDG prompt templates.

**Architecture:** Agent and preset argument lists use an untagged `AgentArg` enum for template strings or expression objects. Launch planning constructs a strict Minijinja environment whose guarded loader exposes the built-in prompt as `builtin/default.jinja`, shadows `default.jinja` from the Terminai XDG config directory, and loads other selected templates from that directory.

**Tech Stack:** Rust, serde/serde_yaml, Minijinja 2.x, xdg, schemars, Cargo tests.

---

### Task 1: Define the new configuration schema

**Files:**
- Modify: `src/terminai_config.rs`
- Test: `src/terminai_config.rs`

1. Add an untagged `AgentArg` enum with `Template(String)` and `Expression { expr: String }` variants.
2. Change agent and preset `args`/`extra_args` to `Vec<AgentArg>` and add useful string conversions for built-in/test construction.
3. Replace the unused `initial_prompt` setting with `prompt_template: Option<String>` on agents and presets.
4. Update Rustdoc to describe Jinja variables, `toml`/`json` filters, expression semantics, and `default.jinja` lookup.
5. Add YAML deserialization tests for mixed string/expression lists and prompt template names.
6. Run `cargo test --manifest-path src/Cargo.toml terminai_config` and confirm it passes.

### Task 2: Migrate bundled configuration and dependency

**Files:**
- Modify: `src/Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `config/codex.yaml`
- Modify: `config/claude.yaml`
- Modify: `config/opencode.yaml`
- Delete: `config/general.yaml`
- Create: `config/default.jinja`

1. Replace the Handlebars dependency with Minijinja.
2. Convert bundled argument interpolation to Jinja syntax and `toml`/`json` filters.
3. Replace conditional omission strings with `expr` entries returning string arrays.
4. Move the general prompt into `default.jinja` and divide it into stable, named blocks while preserving rendered behavior.
5. Update the lockfile with `cargo check --manifest-path src/Cargo.toml`.

### Task 3: Implement Minijinja launch rendering

**Files:**
- Modify: `src/agent_launcher.rs`
- Test: `src/agent_launcher.rs`

1. Replace Handlebars helpers and sentinel parsing with a strict, non-escaping Minijinja environment.
2. Register `toml` and `json` filters.
3. Implement a guarded XDG loader with the `default.jinja` and `builtin/default.jinja` contracts.
4. Carry `prompt_template` through preset inheritance and direct-agent override resolution.
5. Render the selected prompt before args, then render template args or compile/evaluate expression args.
6. Deserialize expression values into `Vec<String>` so arrays with non-string items fail.
7. Replace old Handlebars/sentinel tests with Minijinja tests covering filters, strict undefined handling, zero/multiple expression args, and invalid expression types.
8. Add temp-directory loader tests for default shadowing, explicit built-in extension, selected custom extension of the shadowed default, direct/preset template selection, missing templates, and traversal rejection.
9. Run `cargo test --manifest-path src/Cargo.toml agent_launcher` and confirm it passes.

### Task 4: Update config initialization and public documentation

**Files:**
- Modify: `src/terminai_config_init.rs`
- Modify: `README.md` if configuration examples reference the old syntax
- Modify: `docs/schema-v0.1.5.json`
- Modify: `docs/config.html`

1. Update the generated default-config comments to reference `config/default.jinja` and document `prompt-template`/expression examples where appropriate.
2. Update any checked-in examples or references to `general.yaml`, Handlebars, or old helpers.
3. Run `task config-schema` to regenerate the current schema.
4. Run `task config-docs` to regenerate the HTML documentation.
5. Search the repository (excluding historical design material if necessary) for stale production references.

### Task 5: Verify the complete change

**Files:**
- Review all modified files.

1. Run `cargo fmt --manifest-path src/Cargo.toml -- --check`, formatting first if needed.
2. Run `cargo test --manifest-path src/Cargo.toml`.
3. Run `cargo clippy --manifest-path src/Cargo.toml --all-targets -- -D warnings` if the existing project baseline permits it.
4. Inspect `git diff --check` and `git status --short`.
5. Review the final diff for schema compatibility, loader containment, error context, and accidental unrelated changes.
