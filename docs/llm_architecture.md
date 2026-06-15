# CLI Agent Architecture

**Status:** Current implementation

Termin.AI no longer embeds its own LLM provider client or Python agent. The AI surface is a real PTY-backed terminal that runs the user's configured CLI agent, such as `codex` or `claude`.

## Runtime Shape

```
┌───────────────────────────────┐
│ Termin.AI host process         │
│                                │
│  ┌──────────────┐              │
│  │ Wrapped shell│◄──── input ──┤
│  └──────┬───────┘              │
│         │ terminal VT          │
│  ┌──────▼───────┐              │
│  │ Host MCP     │              │
│  │ read_terminal│              │
│  │ suggest_input│              │
│  └──────┬───────┘              │
│         │ MCP HTTP             │
│  ┌──────▼───────┐              │
│  │ AI CLI PTY   │              │
│  │ codex/claude │              │
│  └──────────────┘              │
└───────────────────────────────┘
```

## Responsibilities

- `src/agent_launcher.rs` builds launch plans for known and custom CLI agents.
- `src/agent_terminal.rs` wraps the AI child process in a terminal PTY.
- `src/mcp_host/` exposes host tools through the official `rmcp` Streamable HTTP server transport.
- `src/agent_tools.rs` carries shell input suggestions into the existing approval flow.
- `src/command/` still classifies suggested input as safe, caution, or dangerous.
- `src/privacy/` filters terminal contents returned by MCP tools.

## MCP Tools

- `read_terminal`: returns visible shell output and recent scrollback.
- `check_for_updates`: returns pending Termin.AI context updates, such as cwd changes, for agents to check before handling each user message.
- `get_terminal_context`: returns cwd, shell, OS, terminal dimensions, mouse mode, and bracketed paste state.
- `suggest_input`: queues exact shell input for user approval.
- `get_suggestion_status`: reports the most recent queued suggestion.

## Security Model

Termin.AI does not own model credentials or provider routing. The configured CLI agent handles auth and model selection. Termin.AI only exposes terminal context and never sends suggested shell input to the wrapped shell without the user's approval.
