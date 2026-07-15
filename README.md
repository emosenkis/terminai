# Terminai

**Interactive Terminal with AI Assistant** - A transparent shell wrapper that provides context-aware AI assistance through an overlay interface.

> **Note:** Terminai borrows terminal virtualization code from [mprocs](https://github.com/pvolok/mprocs) but is a **distinct product** focused on AI-assisted terminal workflows, not multi-process management.

## Overview

Terminai wraps your shell (bash, zsh, fish) and runs your configured AI CLI agent in a secondary terminal. The agent can:
- View your wrapped terminal through Terminai's MCP tools
- Suggest shell input with your approval
- Help debug errors and explain command output
- Answer questions about your current terminal session

**Key Feature:** Press `Ctrl+Space` to activate the AI terminal overlay. It contains the real IO of your configured CLI agent, such as Claude Code or Codex.

## Features

### 🤖 Context-Aware AI Assistant
- Runs your configured CLI agent instead of Terminai owning model/provider logic
- Exposes terminal context through a host MCP server
- Privacy filtering for terminal data returned through MCP
- Works with CLI agents that can load MCP servers, including Claude Code and Codex

### 🛡️ Safety First
- Command approval workflow for dangerous operations
- Safe/Caution/Dangerous command classification
- AI-suggested shell input is sent only after approval
- Terminai does not manage model API keys; your chosen CLI owns auth

### 🎨 Seamless UX
- Transparent operation until AI is invoked
- Overlay is a real terminal running your AI CLI
- Full terminal emulation (vim, htop, colors, etc. all work)
- < 100ms startup overhead

## Installation

### macOS (Homebrew - Recommended)

```bash
# Add the tap and install
brew tap emosenkis/terminai https://github.com/emosenkis/terminai.git
brew install terminai
```

Homebrew now installs prebuilt release binaries from GitHub, so the Rust toolchain is not required on the target machine.

For other Mac installation methods, see [INSTALL_MACOS.md](INSTALL_MACOS.md).

### macOS (Installation Script)

```bash
git clone https://github.com/emosenkis/terminai.git
cd terminai
./scripts/install-macos.sh
```

### Linux / From Source

```bash
git clone https://github.com/emosenkis/terminai.git
cd terminai
cargo build --release -p termin

# Install binary
sudo cp target/release/terminai /usr/local/bin/

```

### Quick Start

1. Install and configure your AI CLI:
```bash
codex login
# or
claude auth
```

2. Create a config file (optional):
```bash
mkdir -p ~/.config/terminai
cp terminai.example.yaml ~/.config/terminai/terminai.yaml
# Edit terminai.yaml to set your preferences
```

3. Launch Terminai:
```bash
terminai
```

4. Use normally until you need AI help, then press `Ctrl+Space`

## Configuration

Terminai no longer stores model provider settings or API keys. Install and authenticate the CLI agent you want to use, then point Terminai at it.

Create `~/.config/terminai/terminai.yaml`:

```yaml
interface:
  chat-position: bottom

agent:
  kind: codex
```

Claude Code example:

```yaml
agent:
  kind: claude
```

Custom agent example:

```yaml
agent:
  kind: custom
  command: my-agent
  args:
    - --mcp-url
    - "{{ mcp_url }}"
    - "{{ context_prompt }}"
    - expr: '["--mcp-enabled"] if uses_mcp else []'
```

Terminai injects a host MCP server and clear context prompt into known agents. The MCP server exposes `check_for_updates`, `read_terminal`, `get_terminal_context`, `suggest_input`, and `get_suggestion_status`.

Agent strings use Minijinja templates. An argument can also be an `expr` object whose Minijinja expression returns an array of strings, allowing one configuration item to produce zero or multiple arguments.

Built-in agent presets are YAML reference configs bundled at build time from `config/codex.yaml`, `config/claude.yaml`, and `config/opencode.yaml`. User `agent-presets` use the same shape and can extend or override those presets.

The built-in prompt is `config/default.jinja`. Place a `default.jinja` in the Terminai XDG config directory to shadow it, or set `agent.prompt-template` to another template in that directory. Templates can extend the shadowable `default.jinja`; a user default can extend `builtin/default.jinja` and override the `introduction`, `general_rules`, `mcp_rules`, `tool_cli_introduction`, or `tool_cli_fallback_rules` block independently:

```jinja
{% extends "builtin/default.jinja" %}
{% block introduction %}Your customized introduction.{% endblock %}
```

## Usage

### Basic Workflow

1. **Normal Terminal Usage**: Use your shell normally - Terminai is transparent
2. **Activate AI**: Press `Ctrl+Space` to open the AI CLI terminal
3. **Ask Questions**: Interact with the CLI agent normally
4. **Command Approval**: Review and approve suggested shell input
5. **Continue**: Press `Ctrl+Space` or `Esc` to close overlay and continue working

### Example Interactions

**Debugging an error:**
```
$ npm run build
Error: Module not found 'react-router-dom'

[Press Ctrl+Space]
You: why did this fail?

AI: The error indicates the 'react-router-dom' package is missing.
    Would you like me to install it?

    Command: npm install react-router-dom
    [Execute] [Edit] [Cancel]
```

**Learning new commands:**
```
[Press Ctrl+Space]
You: find all JavaScript files larger than 1MB

AI: Here's a command to find large JavaScript files:

    Command: find . -name "*.js" -type f -size +1M -exec ls -lh {} \;

    This searches the current directory recursively for .js files
    larger than 1MB and displays their sizes.
    [Execute] [Edit] [Cancel]
```

## Keybindings

| Key | Action |
|-----|--------|
| `Ctrl+Space` | Toggle AI assistant overlay |
| `Ctrl+A` | Toggle focus between process list and terminal |
| `Ctrl+Q` | Quit |
| `Esc` | Close AI overlay |
| `Enter` | Send message to AI (when in AI input) |

See the help window (`?` key) for complete keybindings.

## Development Status

**Current Version:** 0.1.0 (Alpha)

### Completed ✅
- PTY-backed CLI agent overlay
- Command parsing and safety validation
- Privacy filtering
- Terminal virtualization
- AI overlay UI
- Basic integration with app

### In Progress 🚧
- Command execution workflow
- History persistence

### Planned 📋
- Voice input (Whisper API)
- SSH session support
- Plugin system
- Team collaboration features

## Architecture

Terminai consists of:

**Borrowed from mprocs (~30%):**
- `src/vt100/` - Terminal emulation (VT100)
- `src/proc/` - PTY handling
- `src/term/` - Terminal abstractions

**New Terminai code (~70%):**
- `src/agent_launcher.rs` - CLI agent launch planning
- `src/agent_terminal.rs` - AI CLI PTY terminal
- `src/mcp_host/` - Host MCP server for terminal context and suggestions, served with `rmcp`
- `src/command/` - Command parsing, validation, execution
- `src/privacy/` - Sensitive data filtering
- `src/app.rs` - Single-shell application (different from mprocs)

See `IMPLEMENTATION_PLAN.md` for detailed architecture.

## Contributing

This project is in active development. We welcome:
- Bug reports
- Feature requests
- Documentation improvements
- Code contributions

Please read `CLAUDE.md` for guidelines when working with AI assistants on this project.

## Relationship to mprocs

Terminai is **NOT**:
- ❌ A fork of mprocs
- ❌ An extension of mprocs
- ❌ "mprocs with AI added"

Terminai **IS**:
- ✅ A new product with its own vision
- ✅ Using mprocs' terminal virtualization as a code library
- ✅ Building on proven technology to move faster
- ✅ Focused on single-shell + AI assistance

We're grateful to mprocs for their excellent terminal handling code and actively contribute improvements back upstream.

## License

MIT License - see LICENSE file for details.

Portions of terminal virtualization code are from [mprocs](https://github.com/pvolok/mprocs) (MIT License).

## Credits

- **mprocs** by pvolok - Terminal virtualization foundation
- **Ratatui** - Terminal UI framework

---

**Status:** Alpha - Active Development

For questions, issues, or contributions, please visit our [GitHub repository](https://github.com/emosenkis/terminai).
