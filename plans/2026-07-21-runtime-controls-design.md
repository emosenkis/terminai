# Runtime AI Controls Design

## Goal

Add session-level controls for command approval, AI-readable shell history,
and the active CLI agent without taking keyboard shortcuts away from the
wrapped shell.

## Approval modes

Terminai supports two modes:

- `always-ask` queues every suggested input for explicit approval. This is the
  default.
- `auto-approval` sends every suggested input to the wrapped shell without
  consulting the existing risk classifier. The mode is deliberately presented
  as dangerous rather than implying the classifier provides a security
  boundary.

The configured mode is the startup default. In-app changes last only for the
current Terminai session. Enabling auto-approval requires confirmation;
disabling it is immediate. While enabled, the AI overlay status line displays
`⚠ AUTO-APPROVE` in warning colors.

## Runtime controls

Management shortcuts apply only while the AI overlay is active:

- `F7`: toggle approval mode.
- `F8`: open the agent picker.
- `F9`: clear AI-readable shell history after confirmation.
- `F10`: open the control panel.

All bindings are configurable under `interface.key_bindings`. The control
panel exposes approval mode, agent switching, and history clearing with
keyboard navigation. Terminai confirmation dialogs take input priority over
the agent terminal.

## Clearing history

Clearing history truncates Terminai's internal shell VT scrollback and pending
native-scrollback copies while preserving the current visible screen. It does
not erase the terminal emulator's native scrollback or the AI CLI's own
terminal history.

The operation re-synchronizes Terminai's scrollback tracker so removed rows are
not reintroduced or treated as new output. Since `read_terminal` reads the
same internal VT state, the removed history is no longer available to agents.

## Agent switching

The picker lists bundled presets, visible user presets, and the configured
startup agent when it is distinct. `show-in-switcher` on preset configuration
defaults to true.

Terminai validates the selected launch plan before asking for confirmation.
After confirmation it terminates the current agent process, discards that
agent terminal and conversation, and launches a fresh process for the selected
preset. Only one agent process is retained. Runtime selection does not rewrite
`terminai.yaml`.

The active agent selection is retained across working-directory changes and
configuration reloads when it remains valid. If a reloaded configuration
removes the selected user preset, Terminai falls back to the configured startup
agent for the next launch.

## Configuration

Add the top-level startup setting:

```yaml
approval-mode: always-ask # or auto-approval
```

Add these key bindings with the defaults above:

```yaml
interface:
  key_bindings:
    toggle-approval-mode: F7
    switch-agent: F8
    clear-history: F9
    control-panel: F10
```

Agent presets accept:

```yaml
agent-presets:
  private-agent:
    show-in-switcher: false
```

## Error handling

- Invalid or unavailable agent selections leave the current agent running and
  show the error in the picker.
- Failure after the old agent has been terminated leaves the overlay in the
  existing launch-error state so another agent can be selected.
- A failed automatic shell write is logged and does not silently fall back to
  an approval prompt, which could cause the same input to be sent twice.

## Verification

Tests cover configuration defaults and deserialization, switcher visibility,
approval routing, warning/status rendering, confirmation defaults, and VT
history truncation while preserving visible rows. The existing Terminai unit
and end-to-end suites remain the final regression check.
