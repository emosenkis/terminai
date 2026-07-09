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
