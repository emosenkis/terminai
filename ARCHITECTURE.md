# Architecture Notes

## Native Scrollback Injection

Termin.AI keeps the wrapped shell in an internal VT100 model and renders that
model through ratatui. When new rows enter the internal VT100 scrollback,
Termin.AI mirrors those rows into the host terminal's native scrollback so
users can scroll with their terminal emulator.

There is no standard terminal escape sequence for "append these arbitrary rows
to native scrollback". Native scrollback is emulator state outside the VT
screen model. The only portable way to make a terminal add rows to native
scrollback is to let the main screen scroll normally.

### Scrolling Region Compatibility

The first native-scrollback implementation drew rows at the top of the screen
and then used a scrolling region command:

```text
CSI top;bottom r
CSI n S
CSI r
```

That works on Konsole/Yakuake in testing, and similar behavior has been
reported for some xterm-like terminals, but it is not portable. Some terminals
scroll the visible grid without appending the displaced rows to native
scrollback.

Known evidence:

- xterm documents the normal and alternate screen buffers as separate buffers;
  the normal screen can have saved lines, while the alternate screen is
  display-sized and has no saved lines. This means alternate-screen behavior
  cannot be assumed to populate main-screen scrollback.
  <https://www.xfree86.org/current/ctlseqs.html>
- Apple Terminal documents the same split: the main screen contains a log of
  output, while the alternate screen is for full-screen interactive apps.
  <https://support.apple.com/guide/terminal/display-or-hide-the-alternate-screen-trmld1f46097/mac>
- iTerm2 exposes saving alternate-screen lines to scrollback as profile policy,
  not as guaranteed VT behavior. It also has a separate policy for saving lines
  when an app status bar is present.
  <https://iterm2.com/documentation-preferences-profiles-terminal.html>
- Microsoft Terminal tracked a closely related bug where scrolling regions that
  touched the top of the screen did not place scrolled-off lines into the
  scrollback buffer.
  <https://github.com/microsoft/terminal/issues/3673>
- Kitty issue 3113 contains a minimal reproduction using a scrolling region:
  visible content scrolls, but native scrollback does not receive the lines on
  some terminals. The discussion also records emulator differences, including
  Konsole, xterm, Apple Terminal, and iTerm2.
  <https://github.com/kovidgoyal/kitty/issues/3113>

### Current Mechanism

Termin.AI now mirrors rows into native scrollback without scrolling regions:

1. Reset host terminal scroll margins (`CSI r`) and origin mode (`CSI ?6 l`),
   then move the real terminal cursor to the top-left corner.
2. Stream each row that should enter native scrollback as normal terminal
   output: apply attributes, print the row's visible cells left-to-right, then
   emit a line advance.
3. Emit enough additional line advances to reach the bottom of the screen and
   scroll exactly the streamed rows into native scrollback.
4. Treat the visible terminal area as blank after this streaming pass.
5. Redraw the intended current frame from that blank state.

Resetting margins before the stream matters because a stale scrolling region can
make the final line advances scroll only that region. On macOS Terminal and
iTerm2, that can move the visible grid while bypassing the main native
scrollback, which looks like the current screen advanced but the scrollback
still ends with rows from an older command.

This makes the terminal perform normal main-screen linefeed scrolling, which is
the behavior terminal emulators use to append rows to native scrollback. It also
avoids using absolute cursor addressing for the rows that are meant to become
native scrollback history; absolute positioning is limited to the initial move
to the top-left corner before streaming begins.

### Remaining Constraints

Native scrollback requires the host to be on the normal screen buffer and to
honor normal main-screen linefeed scrolling. Termin.AI should not enter the
alternate screen for the wrapped shell viewport, and it should not use
scroll-region operations for rows that must enter native scrollback.
