# CLI Agent MCP Terminal Implementation Plan

**Goal:** Replace Termin.AI's built-in AG-UI/Python LLM agent with a user-configured CLI agent running inside a real terminal pane, with Termin.AI exposing host capabilities through an MCP server.

**Architecture:** Termin.AI should stop acting as an LLM provider/client. It should spawn an interactive AI CLI in a PTY, render that PTY as the AI widget, and pass startup flags/config that inject general context and a host-provided MCP server. The host MCP server owns terminal-aware tools such as reading shell contents and suggesting terminal input, preserving Termin.AI's approval UX without maintaining its own chat protocol.

**Tech Stack:** Rust, portable-pty, ratatui, existing `Shell`/VT100 parser, JSON-RPC/MCP over stdio or local HTTP, configured external CLIs such as `claude` and `codex`.

---

## Current State

The current AI path is spread across:

- `src/bin/terminai.rs`: initializes `AIChatProcess`, extracts shell context, renders `AIChatUI`, routes overlay input, handles tool events and command approval.
- `src/ai_proc/chat_process.rs`: owns chat state, AG-UI message history, streaming state, tool coordination, command suggestions, and privacy filtering.
- `src/ai_proc/ui.rs`: renders the custom chat overlay, tool cards, input widget, errors, and approval dialogs.
- `src/llm/*`: AG-UI client/subscriber/tool coordinator/provider abstractions.
- `src/llm_subprocess.rs` and `python/terminai_agent/*`: spawn and serve the Python LLM agent.
- `src/llm/tool_executor.rs`: useful host-side tool behavior to keep, especially `read_scrollback` and `suggest_command`.
- `src/shell.rs`: direct PTY-backed shell abstraction currently used by the main `terminai` binary.
- `src/proc/*`: older mprocs-style PTY process abstraction that is more generic but is not the primary path in `terminai`.

The target design should delete most of `src/llm/*`, `src/ai_proc/*`, `src/llm_subprocess.rs`, and `python/terminai_agent/*`, replacing them with a much smaller `agent_terminal` plus `mcp_host` layer.

## Target User Experience

Pressing `Ctrl+Space` shows the AI terminal. That terminal is not a custom chat widget; it is the actual TTY UI of `claude`, `codex`, or another configured CLI. Keys, paste, mouse events, resizing, alternate screen behavior, and scrollback should behave like they do for the main shell terminal.

Termin.AI still provides host awareness through MCP tools:

- `read_terminal`: return visible screen and recent scrollback from the wrapped shell.
- `suggest_input`: ask Termin.AI to offer a command/input approval prompt for the wrapped shell.
- `get_terminal_context`: return cwd, shell, OS info, dimensions, and last known output.
- optional later: `send_input_after_approval`, `list_sessions`, `focus_shell`, `read_ai_terminal`.

The external agent handles model choice, auth, tool protocol, conversation history, and streaming UI. Termin.AI only launches it and exposes terminal-aware capabilities.

## Configuration Model

Replace provider/model config with CLI-agent config in `src/terminai_config.rs`.

Suggested YAML:

```yaml
interface:
  chat-position: bottom

agent:
  command: codex
  args:
    - --cd
    - "{cwd}"
    - --sandbox
    - workspace-write
    - --ask-for-approval
    - on-request
  initial-prompt: |
    You are running inside Termin.AI.
    Use the terminai MCP server to inspect the user's wrapped terminal.
    Suggest terminal input instead of writing directly to the wrapped shell.
  mcp:
    mode: stdio
    strict: true
```

For Claude Code, the adapter can produce:

```bash
claude \
  --append-system-prompt "$TERMINAI_CONTEXT_PROMPT" \
  --mcp-config "$TERMINAI_MCP_CONFIG" \
  --strict-mcp-config \
  --permission-mode default
```

For Codex, based on current local CLI help, the adapter can produce:

```bash
codex \
  --cd "$PWD" \
  --sandbox workspace-write \
  --ask-for-approval on-request \
  -c 'mcp_servers.terminai.command="terminai"' \
  -c 'mcp_servers.terminai.args=["mcp-host","--session","..."]' \
  "$TERMINAI_CONTEXT_PROMPT"
```

The exact Codex config keys must be verified against the installed Codex config format during implementation. If runtime `-c mcp_servers...` is too brittle, write a temporary config/profile file and launch with `--profile terminai-<session>`.

## Design Decisions

1. Use a real PTY for the AI child process.
   This aligns with the user's requirement that the widget be "just another terminal" and avoids reimplementing each CLI's interactive UI.

2. Keep Termin.AI command approval outside the AI CLI.
   The MCP `suggest_input` tool should store a pending suggestion in host state, and Termin.AI should render an approval prompt outside or above the AI terminal. Approval sends bytes to the wrapped shell, not to the AI terminal.

3. Prefer per-session MCP injection over mutating user config.
   Do not permanently edit `~/.claude`, `~/.codex`, or project `.mcp.json` just to run Termin.AI. Generate ephemeral config files or CLI `-c` overrides.

4. Start with stdio MCP if possible, but keep transport abstract.
   Stdio is easiest for Claude/Codex config, but it is awkward when Termin.AI is already the parent process and needs bidirectional access to live host state. If stdio child lifetime becomes tangled with the AI CLI, use a localhost streamable HTTP MCP server and pass URL-based config to CLIs that support it.

5. Keep privacy filtering.
   Move privacy filtering out of `AIChatProcess` and into MCP tool handlers, so all terminal contents returned to CLI agents pass through the same filter.

## Implementation Tasks

### Task 1: Introduce Agent Config Types

**Files:**
- Modify: `src/terminai_config.rs`
- Modify: `README.md`
- Test: config unit tests in `src/terminai_config.rs`

Steps:

1. Add `AgentConfig`, `McpConfig`, and `AgentKind` or adapter fields.
2. Deprecate `providers` and `default_model` in code paths used by `terminai`.
3. Preserve `interface.chat-position` and key bindings.
4. Add tests for minimal config, custom command/args, Claude preset, Codex preset, and missing command.
5. Keep backward compatibility only if cheap: if old provider config exists, produce a clear migration error rather than launching AG-UI.

### Task 2: Create an AI PTY Process

**Files:**
- Create: `src/agent_terminal.rs`
- Modify: `src/lib.rs`
- Modify: `src/main.rs`
- Modify: `src/bin/terminai.rs`
- Test: new unit tests or integration harness around process spawn config

Steps:

1. Extract reusable PTY behavior from `src/shell.rs` or wrap `Shell::spawn_command`.
2. Add `AgentTerminal` with VT parser, writer, resize, key, paste, command spawn metadata, and exit status.
3. Give the AI PTY its own scrollback length and terminal dimensions based on overlay size.
4. On overlay resize, resize the AI PTY to `overlay_area`.
5. Route keys/mouse/paste to the AI PTY while the overlay is visible.
6. Remove the custom `AIChatUI` input focus flow from the active render/event path.

### Task 3: Render the AI Widget as a Terminal

**Files:**
- Modify: `src/bin/terminai.rs`
- Possibly modify: `src/ui_term.rs` or `src/ui_layer/terminal_layer.rs`
- Test: `src/tests/e2e/test_ai.rs`, snapshots

Steps:

1. Replace `state.ai_ui.render(...)` with `TerminalWidget::with_offset(agent_vt.screen(), 0)`.
2. Keep `Clear.render(overlay_area, buf)` and the current bottom/top overlay layout initially.
3. Use the AI terminal screen cursor when overlay is visible.
4. Show a small fallback message only when the configured CLI cannot be found or fails to spawn.
5. Update e2e snapshots from "AI Assistant" chat UI to PTY-rendered AI output.

### Task 4: Add Host MCP Server Core

**Files:**
- Create: `src/mcp_host/mod.rs`
- Create: `src/mcp_host/server.rs`
- Create: `src/mcp_host/tools.rs`
- Modify: `src/lib.rs`
- Modify: `src/Cargo.toml`
- Test: `src/tests/test_mcp_host.rs`

Steps:

1. Pick a Rust MCP implementation after a small dependency spike. Prefer a maintained crate if it supports server-side tools over stdio and/or streamable HTTP; otherwise implement the minimal JSON-RPC methods needed: `initialize`, `tools/list`, `tools/call`, and shutdown.
2. Define a `TerminaiMcpState` holding thread-safe access to shell VT, cwd/context provider, privacy filter, and a command-suggestion sender.
3. Implement `read_terminal` with arguments `{ "max_lines": number, "include_visible": bool }`.
4. Implement `get_terminal_context` with cwd, shell, OS, terminal dimensions, and whether the wrapped terminal has mouse/bracketed paste enabled.
5. Implement `suggest_input` with `{ "input": string, "explanation": string, "target": "shell" }`.
6. Return structured JSON and concise text content for compatibility with different MCP clients.

### Task 5: Bridge MCP Suggestions to Approval UX

**Files:**
- Create: `src/agent_tools.rs` or reuse `src/command/*`
- Modify: `src/bin/terminai.rs`
- Modify: `src/command/parser.rs` only if needed
- Test: approval dialog tests in `src/tests/e2e/test_ai.rs`

Steps:

1. Move `PendingCommand` out of `src/ai_proc/chat_process.rs` into a host-owned module.
2. Reuse `SafetyValidator` to classify suggested input.
3. Reuse existing approve/deny key bindings.
4. On approve, send the decoded input to `state.shell.send_command(...)`.
5. On deny, notify the MCP tool call result that the suggestion was rejected if the MCP transport supports waiting. If not, return "suggestion queued for user approval" immediately and expose a later `get_suggestion_status` tool.

### Task 6: Build CLI Agent Adapters

**Files:**
- Create: `src/agent_launcher.rs`
- Modify: `src/terminai_config.rs`
- Modify: `src/bin/terminai.rs`
- Test: launcher unit tests

Steps:

1. Define `AgentLaunchPlan { command, args, env, cwd, temp_files }`.
2. Implement generic launch from config without provider assumptions.
3. Implement `claude` adapter:
   - pass `--append-system-prompt` or `--system-prompt` based on config
   - pass `--mcp-config <temp-json>`
   - pass `--strict-mcp-config` when configured
4. Implement `codex` adapter:
   - pass `--cd <cwd>`
   - pass `--sandbox workspace-write` by default
   - pass `--ask-for-approval on-request` by default
   - inject MCP through `-c` overrides or a temporary profile/config file after verifying supported config format
5. For unknown commands, support raw args plus template expansion for `{cwd}`, `{mcp_config}`, `{mcp_url}`, and `{context_prompt}`.
6. Validate executable availability using `which`.

### Task 7: Remove AG-UI and Python Agent Path

**Files:**
- Delete or stop compiling: `src/llm_subprocess.rs`
- Delete or stop compiling: `src/llm/client.rs`, `src/llm/subscriber.rs`, `src/llm/tool_coordinator.rs`, `src/llm/forwarded_props.rs`, `src/llm/providers.rs`
- Delete or archive: `python/terminai_agent/*`
- Modify: `src/llm/mod.rs` or remove module entirely
- Modify: `src/Cargo.toml`
- Modify: `src/main.rs`, `src/lib.rs`
- Modify tests under `src/tests/test_llm_*` and `src/tests/test_agui_tool_e2e.rs`

Steps:

1. Remove `ag-ui-client`, `ag-ui-core`, `reqwest` if no longer needed, and Python-agent-only dependencies.
2. Keep or relocate `TerminalContext`, `ToolExecutor` useful logic, and privacy filtering.
3. Delete AG-UI tests and replace with MCP host tests.
4. Remove Python packaging docs from the primary install path.
5. Ensure `cargo test` compiles without the `python/` agent.

### Task 8: Update Docs and Migration Messaging

**Files:**
- Modify: `README.md`
- Modify: `docs/llm_architecture.md`
- Possibly create: `docs/agent-cli.md`
- Modify: `python/README.md` or remove it from docs index

Steps:

1. Rewrite "multi-provider support" as "bring your own CLI agent".
2. Document Claude and Codex examples.
3. Explain the MCP tools Termin.AI provides.
4. Document security model: Termin.AI does not hold API keys; the chosen CLI owns auth; suggested shell input still requires user approval.
5. Add troubleshooting for missing CLI, MCP startup failure, and broken flags after CLI upgrades.

### Task 9: Verification Pass

Commands:

```bash
cargo test -p termin
cargo test -p termin --features snapshot-tests
cargo clippy -p termin --all-targets -- -D warnings
cargo run -p termin --bin terminai
```

Manual checks:

1. Launch with `agent.command: claude`; verify overlay shows Claude's real TUI.
2. Launch with `agent.command: codex`; verify overlay shows Codex's real TUI.
3. Ask the agent to inspect terminal output; verify it uses `read_terminal`.
4. Ask the agent to suggest a command; verify Termin.AI shows approval and sends input to the wrapped shell only after approval.
5. Resize terminal; verify both shell and AI PTYs resize correctly.
6. Paste into overlay; verify paste goes to the AI CLI.
7. Close and reopen overlay; decide whether the AI process remains alive. Recommended default: keep alive for session continuity.

## Suggested Commit Sequence

1. `config: add cli agent configuration`
2. `agent: spawn ai cli in terminal overlay`
3. `mcp: expose terminal context tools`
4. `agent: wire command suggestions to approval flow`
5. `agent: add claude and codex launch adapters`
6. `cleanup: remove ag-ui python agent`
7. `docs: document cli agent architecture`

## Risks

- CLI flags are not stable across Claude/Codex versions. Keep adapters small, version-tolerant, and overrideable with raw user args.
- MCP transport differs by client. Implement transport behind an interface and test both a temp JSON config path and raw config overrides.
- Interactive CLIs may use alternate screen, mouse capture, or bracketed paste. Treat the AI child exactly like a terminal program and avoid custom chat key handling.
- Approval UX can become ambiguous if both the AI CLI and Termin.AI request approvals. Make the Termin.AI prompt visually distinct and only for wrapped-shell input.
- Some agents may already have shell tools. The system prompt should instruct them to use Termin.AI MCP for the wrapped terminal, but Termin.AI cannot fully prevent external agent behavior unless the selected CLI supports tool restrictions.

## Open Questions

1. Should the AI terminal be spawned at app startup or lazily on first `Ctrl+Space`? Recommended: lazy spawn, then keep alive.
2. Should Termin.AI offer presets (`agent: claude`, `agent: codex`) or require raw command/args? Recommended: both, with presets expanding to editable launch plans.
3. Should `suggest_input` block until approval or return immediately? Recommended: immediate return for v1, then add status polling if agents need feedback.
4. Should old provider config be migrated automatically? Recommended: no automatic migration; produce a clear error with example new config.
