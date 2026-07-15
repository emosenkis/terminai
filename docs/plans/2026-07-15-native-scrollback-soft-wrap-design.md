# Native Scrollback Soft-Wrap Design

## Goal

Preserve logical lines in host-terminal native copy when the internal VT100
screen wraps a line across physical rows.

## Root cause

The VT100 `Row` records whether its content continues onto the next physical
row through `Row::wrapped()`. That distinction must survive both paths that
write the VT model to the host terminal: native scrollback streaming and the
subsequent visible-frame redraw. Losing it in either path makes the host record
separate logical lines at the wrap boundary.

## Design

Carry one wrap flag per row alongside the cells in a native scrollback
snapshot. Extend the Ratatui frame snapshot and backend interface to accept
these flags.

When streaming a row:

- If it is not wrapped, emit `\r\n` as today.
- If it is wrapped, do not move the cursor or emit a line break. Printing the
  next row's first character will trigger the host terminal's pending automatic
  wrap and preserve the logical line for native selection and copy.

If the final snapshot row is wrapped, its continuation is still in the visible
VT screen rather than in the snapshot. Trigger that pending host wrap with a
temporary space, then backspace and erase it. This advances the physical row
without creating a hard line ending; the subsequent frame redraw supplies the
real continuation content.

The final blank-line advances used to push streamed rows into native
scrollback remain unchanged. A full-width row cannot be used to infer wrapping
because it may be followed by a real newline; the VT100 flag is authoritative.

The visible-frame buffer also marks the last drawable cell of each wrapped VT
row. During ordinary Ratatui drawing, a transition from that cell to column
zero of the following physical row is emitted without absolute cursor
positioning, allowing the next printable cell to trigger host auto-wrap. Rows
without the marker retain the normal absolute move, including full-width rows
that end with a real newline.

## Verification

Add focused tests showing that a soft-wrapped row is streamed without an
intervening `\r\n`, its visible redraw contains no absolute row transition, and
a full-width row with a hard line ending retains that transition. Cover the
tail-like case where newly arriving output turns the previous visible row into
a soft-wrapped continuation while another row enters native scrollback.
