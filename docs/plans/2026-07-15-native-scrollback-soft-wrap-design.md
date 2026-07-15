# Native Scrollback Soft-Wrap Design

## Goal

Preserve logical lines in host-terminal native copy when the internal VT100
screen wraps a line across physical rows.

## Root cause

The VT100 `Row` records whether its content continues onto the next physical
row through `Row::wrapped()`. Native scrollback snapshots currently retain only
the rendered cells, so this distinction is lost before the rows reach the
Crossterm backend. The backend consequently emits `\r\n` after every physical
row, causing copied text to contain newlines at soft-wrap boundaries.

## Design

Carry one wrap flag per row alongside the cells in a native scrollback
snapshot. Extend the Ratatui frame snapshot and backend interface to accept
these flags.

When streaming a row:

- If it is not wrapped, emit `\r\n` as today.
- If it is wrapped, do not move the cursor or emit a line break. Printing the
  next row's first character will trigger the host terminal's pending automatic
  wrap and preserve the logical line for native selection and copy.

The final blank-line advances used to push streamed rows into native
scrollback remain unchanged. A full-width row cannot be used to infer wrapping
because it may be followed by a real newline; the VT100 flag is authoritative.

## Verification

Add focused tests showing that a soft-wrapped row is streamed without an
intervening `\r\n`, while a full-width row with a hard line ending retains the
explicit `\r\n`. Existing native-scrollback and workspace tests must continue
to pass.
