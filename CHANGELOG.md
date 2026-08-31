## 0.1.24 - 2026-08-31

- Replace eager command insertion with debounced, explicitly accepted ghost-text
  completions, multiple candidates, and a configurable manual key sequence.
- Document OSC 133/633 setup for Fish, Bash, Zsh, terminal shell integrations,
  and Starship.

## 0.1.23 - 2026-08-31

- Fix incorrect guest sizing when Terminai is the topmost program in a graphical
  terminal by querying the host, reconciling after PTY startup, and answering
  guest size queries locally.
- Bound pre-launch terminal-size detection to 250 ms and always launch the guest
  with a safe fallback if the host does not answer.

## 0.1.22 - 2026-08-31

- Configure command completion independently from the interactive agent with
  dedicated `auto-completer` and `auto-completers` settings and built-in Codex,
  Claude, and OpenCode presets.

## 0.1.21 - 2026-08-29

- Add off-by-default AI command completion using the active agent's
  configurable single-prompt invocation, with OSC 133/633 prompt detection,
  session controls, privacy filtering, and stale-response protection.

## 0.1.20 - 2026-07-29

- Add capability-gated synchronized terminal output.
- Fix style corruption when using native scrollback.

## 0.1.19 - 2026-07-29

- Render changelogs without a Markdown dependency and automatically show only
  releases newer than the last acknowledged version after an upgrade.

## 0.1.18 - 2026-07-28

- Add session-only CLI overrides for approval mode, active agent, AI terminal
  position and height, and guest display mode.

## 0.1.17 - 2026-07-28

- Add a wrapped, scrollable changelog to Terminai Controls.
- Show release notes automatically after upgrades, with an opt-out
  `changelog: false` setting.

## 0.1.16 - 2026-07-28

- Expand shell-input approval dialogs to the available screen size to reduce
  wrapping and scrolling.
- Display control characters as distinct keyboard-style labels such as
  `<C-c>`, `<Esc>`, `<Enter>`, and `<Tab>`.
- Fix formatting violations that prevented the Windows workflow from running.

## 0.1.15 - 2026-07-27

- Add support for iTerm2 OSC 1337 escape sequences, including pass-through to host terminal, CWD tracking (`CurrentDir`), and scrollback clearing (`ClearScrollback`).

## 0.1.14 - 2026-07-27

- Report whether suggested shell input needs user approval or was
  auto-approved, and keep MCP suggestion status synchronized with approval
  decisions.

## 0.1.13 - 2026-07-24

- Add configurable AI terminal height, top/bottom/fullscreen positions, and
  resize/overlay/move guest display modes.
- Consolidate layout controls into Layout Mode and Terminai Controls, with
  dedicated shortcuts for settings, layout mode, and fullscreen.
- Replace the AI terminal's full border with a directional separator and make
  fullscreen borderless.
- Preserve the correct guest rows when moving it above or below the AI terminal.

## 0.1.12 - 2026-07-22

- Keep the AI overlay open when auto-approval sends input, while preserving
  the existing close-on-explicit-approval behavior.

## 0.1.11 - 2026-07-21

- Add session-level always-ask and dangerous auto-approval modes, an overlay
  control panel, configurable overlay-only management shortcuts, internal
  history clearing, and confirmed switching between configured AI agents.

## 0.1.10 - 2026-07-17

- Replace fixed terminal redaction with configurable privacy filters powered by Redact.
- Prevent a panic when a terminal briefly reports a zero-sized window during resize or startup.

## 0.1.9 - 2026-07-17

- Prevent a panic when a terminal briefly reports a zero-sized window during resize.

## 0.1.8 - 2026-07-16

- Recover from Terminai errors and panics by resetting the terminal and starting the wrapped command during startup or an interactive shell after startup.

## 0.1.7 - 2026-07-16

- Add experimental Windows build, packaging, shell-selection, and terminal integration groundwork. Windows remains unqualified pending required CI and human QA.
- Add Windows release artifacts with checksum-pinned Scoop manifests.
- Replace legacy application-only dependencies and paths with Terminai-owned runtime paths.

## 0.1.6 - 2026-07-15

- Use Minijinja for agent argument templates, including Jinja expressions that can expand to multiple arguments.
- Preserve soft-wrapped terminal lines during native scrolling and redraw so copying and line selection do not insert spurious newlines.

## 0.1.5 - 2026-07-13

- Add configurable Terminai MCP and CLI tool integration flags for agent presets.
- Expose Terminai tool and MCP launch commands to agent templates and prompts.
- Document the updated Handlebars config variables and regenerate the versioned config schema.
- Add kebab-case YAML config keys and CLI tool aliases.
- Make `terminai --version` report the binary name.

## 0.1.4 - 2026-07-09

- Add a hidden `terminai tool` CLI for calling Terminai MCP tools directly from agents and shell pipelines.

## 0.1.3 - 2026-07-09

- Properly handle lines wrappings during screen resize
- Configure bundled Codex and Claude presets to connect directly to Terminai's HTTP MCP server with bearer-token authorization.

## 0.1.2 - 2026-07-09

- Route bundled Codex and Claude MCP integrations through Terminai's hidden stdio proxy.
- Protect the local HTTP MCP server with a generated per-launch bearer token.

## 0.1.1 - 2026-07-09

- Close the AI modal after an approved shell input suggestion is sent.
- Title the AI modal with the launched agent command name.
- Keep the AI terminal content visible after agent exit and append the exit status plus relaunch hint at the bottom.

## 0.1.0 - 2026-07-06

First public release
