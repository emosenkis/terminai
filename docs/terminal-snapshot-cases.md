# Terminal snapshot test cases

Implement these after the reusable emulator harness. Use the same deterministic
fake shell and fake agent commands across emulators. `All` means internal VT,
tmux, Zellij, and ghostty-vt; narrower coverage is called out where it provides
distinct value.

| Priority | Case | Final assertion | Emulators | Scrollback |
| --- | --- | --- | --- | --- |
| P0 | Wrapped command happy path | Typed input, prompt, styled stdout/stderr, and cursor end in the expected cells | All | No |
| P0 | Open and close the AI overlay | Guest contents survive `Ctrl+Space`, agent pane has the configured separator, and the deactivate binding restores the guest | All | No |
| P0 | Bottom resize split | Guest reflows above a 50% agent pane without stale cells | All | No |
| P0 | Top move split | Unchanged guest rows are shifted/cropped below the agent pane | All | No |
| P0 | Overlay and fullscreen layouts | Overlay leaves guest geometry unchanged; fullscreen fully replaces it | All | No |
| P0 | Approve suggested input | Safe, caution, and dangerous commands render their styles; `y` closes the dialog and writes exact bytes to the guest | All | No |
| P0 | Deny suggested input | `n` closes the dialog without changing the guest prompt or command line | All | No |
| P0 | Native history streaming | More than one viewport of colored hard-broken lines remains ordered and styled | tmux, Zellij | Yes |
| P0 | Soft-wrapped history | Copy-visible logical lines do not gain hard newlines at viewport boundaries | All | Yes |
| P0 | Resize with history and overlay | Narrow/wide resize preserves history, wrap metadata, cursor, and the lower render band | All | Yes |
| P0 | Alternate screen round trip | Full-screen child output is correct; exiting restores the primary screen and its history | All | Yes |
| P1 | Layout Mode controls | `+`, `-`, `p`, `g`, and `f` update height, position, guest mode, and fullscreen labels and geometry | All | No |
| P1 | Control panel | Approval mode, agent picker, history clearing, and confirmation dialogs render focused/selected states | All | No |
| P1 | Agent exit and relaunch | Existing agent content remains, exit status and relaunch hint appear, then relaunch clears only the agent pane | All | No |
| P1 | Approval text encoding | Spaces, tabs, newlines, Escape, Ctrl-C, and non-ASCII input render unambiguously and approve byte-for-byte | All | No |
| P1 | Unicode cell boundaries | Combining marks, CJK wide cells, emoji ZWJ sequences, and a wide glyph in the last column do not leave half-cells | internal, ghostty-vt, tmux | No |
| P1 | Scroll burst beyond internal cap | Host history receives the bounded pending stream in order while the visible tail remains correct | internal, tmux, Zellij | Yes |
| P1 | Clear AI-readable history | Internal readable history clears without erasing the visible guest or host-native history | tmux, Zellij | Yes |
| P1 | OSC state changes | OSC 7 CWD, OSC 8 hyperlinks, palette changes, and OSC 1337 clear-scrollback affect only their intended state | internal, ghostty-vt, tmux | Yes where clearing is tested |
| P1 | Terminal mode input | Application cursor keys, bracketed paste, focus reporting, and mouse mode send the expected bytes after overlay toggles | All | No |
| P2 | Minimum and transient zero size | Small supported viewports render without panic; a zero-size resize recovers on the next valid size | internal, tmux | No |
| P2 | Rapid output during overlay changes | Guest output arriving while opening, moving, resizing, and closing the overlay leaves no stale frame | All | Yes |
| P2 | Synchronized updates | DEC 2026 enabled/disabled paths converge on the same styled final state without leaked mode state | internal, ghostty-vt, tmux | No |
| P2 | Partial and malformed escape streams | Split UTF-8/CSI/OSC sequences and invalid bytes cannot corrupt later valid rendering | internal, ghostty-vt | No |
| P2 | Completion race | Prompt-marker completion appears once; typing or pasting before the reply prevents stale completion insertion | All | No |
| P2 | Agent switch confirmation | Cancel preserves the old session; confirm terminates it and renders the new preset from a clean agent viewport | All | No |

Start with one representative P0 scenario per row. Existing unit tests already
cover combinatorial parser, resize, and widget details; add emulator snapshots
only where the composed terminal state or real interaction can regress.
