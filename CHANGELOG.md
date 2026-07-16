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
