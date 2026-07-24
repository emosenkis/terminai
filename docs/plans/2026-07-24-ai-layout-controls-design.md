# AI Layout Controls Design

## Behavior

Terminai exposes three AI layouts:

- `top` and `bottom` use a configurable height percentage, defaulting to 50%.
- `fullscreen` occupies the entire terminal, has no border, and hides the guest.

Non-fullscreen layouts draw only the edge adjoining the guest. That separator
contains `↑ agent` when AI is above the guest and `↓ agent` when AI is below it,
plus right-aligned status such as `⚠ AUTO-APPROVE`.

The guest display mode defaults to `resize`:

- `resize` resizes the guest PTY to the remaining space.
- `overlay` leaves the guest PTY and viewport unchanged beneath the AI.
- `move` leaves the guest PTY full-sized and shifts/crops its viewport away from
  the AI.

## Controls

- `F10` opens Terminai Controls.
- `F11` toggles fullscreen.
- `F9` enters or leaves Layout Mode.

The old direct F-key bindings for approval, agent switching, and history
clearing are removed. Those actions remain in Terminai Controls.

Layout Mode shows current layout values and supports both selection/navigation
and direct keys: `+`/`-` adjust height by five percentage points within 20–80%,
`p` toggles top/bottom, `g` cycles guest mode, `f` toggles fullscreen, and
`Esc` exits. Terminai Controls exposes fullscreen directly and opens Layout
Mode for the remaining layout settings.

Runtime changes last for the session. Configuration supplies startup values and
configurable bindings for the three global controls.

## Implementation

Reuse `AppState`, the existing control modal, Ratatui layout primitives, and
the current key-binding configuration. Centralize geometry in the existing
overlay area helpers so rendering, PTY sizing, cursor translation, mouse
translation, startup, and resize events use the same dimensions.

Tests cover configuration, layout geometry, modal navigation, and border/status
rendering. Existing unit and end-to-end tests provide regression coverage.
