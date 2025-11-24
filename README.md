# Termin.AI

**Interactive Terminal with AI Assistant** - A transparent shell wrapper that provides context-aware AI assistance through an overlay interface.

> **Note:** Termin.AI borrows terminal virtualization code from [mprocs](https://github.com/pvolok/mprocs) but is a **distinct product** focused on AI-assisted terminal workflows, not multi-process management.

## Overview

Termin.AI wraps your shell (bash, zsh, fish) and adds an AI assistant that can:
- View your terminal history and understand context
- Suggest and execute commands with your approval
- Help debug errors and explain command output
- Answer questions about your current terminal session

**Key Feature:** Press `Ctrl+Space` to activate the AI overlay - it appears over your terminal without disrupting your workflow.

## Features

### 🤖 Context-Aware AI Assistant
- Automatically captures terminal history
- Privacy filtering for sensitive data (API keys, passwords)
- Multi-provider support (Anthropic Claude, OpenAI GPT-4, Google Gemini, Ollama)

### 🛡️ Safety First
- Command approval workflow for dangerous operations
- Safe/Caution/Dangerous command classification
- Edit commands before execution
- Never logs or displays API keys

### 🎨 Seamless UX
- Transparent operation until AI is invoked
- Overlay interface preserves terminal visibility
- Full terminal emulation (vim, htop, colors, etc. all work)
- < 100ms startup overhead

## Installation

### From Source (Rust)

```bash
git clone https://github.com/yourusername/termin.ai.git
cd termin.ai
cargo build --release
sudo cp target/release/termin /usr/local/bin/terminai
```

### Quick Start

1. Set up your API key:
```bash
export ANTHROPIC_API_KEY="your-api-key-here"
```

2. Create a config file (optional):
```bash
mkdir -p ~/.config/terminai
cp terminai.example.yaml ~/.config/terminai/config.yaml
# Edit config.yaml to set your preferences
```

3. Launch Termin.AI:
```bash
terminai
```

4. Use normally until you need AI help, then press `Ctrl+Space`

## Configuration

### Minimal Configuration

Create `~/.config/terminai/config.yaml`:

```yaml
ai:
  enabled: true
  provider: anthropic  # or: openai, gemini, ollama
  model: claude-3-5-sonnet-20241022  # optional
  api_key_env: ANTHROPIC_API_KEY
```

### Supported Providers

| Provider | Models | API Key Env Var |
|----------|--------|-----------------|
| **Anthropic** | claude-3-5-sonnet-20241022, claude-3-opus-20240229 | `ANTHROPIC_API_KEY` |
| **OpenAI** | gpt-4-turbo, gpt-3.5-turbo | `OPENAI_API_KEY` |
| **Google** | gemini-pro, gemini-flash | `GOOGLE_API_KEY` |
| **Ollama** | llama3.2, codellama (local) | None (local) |

### Full Configuration Example

See `terminai.example.yaml` for all available options including:
- Privacy filters
- Command safety rules
- UI preferences
- Keybindings

## Usage

### Basic Workflow

1. **Normal Terminal Usage**: Use your shell normally - Termin.AI is transparent
2. **Activate AI**: Press `Ctrl+Space` to open the AI overlay
3. **Ask Questions**: Type your question or request
4. **Command Approval**: Review and approve/edit suggested commands
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
- LLM client with multi-provider support
- Command parsing and safety validation
- Privacy filtering
- Terminal virtualization
- AI overlay UI
- Basic integration with app

### In Progress 🚧
- Command execution workflow
- Streaming AI responses in UI
- Input handling in AI overlay
- History persistence

### Planned 📋
- Voice input (Whisper API)
- SSH session support
- Plugin system
- Team collaboration features

## Architecture

Termin.AI consists of:

**Borrowed from mprocs (~30%):**
- `src/vt100/` - Terminal emulation (VT100)
- `src/proc/` - PTY handling
- `src/term/` - Terminal abstractions

**New Termin.AI code (~70%):**
- `src/llm/` - Multi-provider LLM client
- `src/ai_proc/` - AI chat process and UI
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

Termin.AI is **NOT**:
- ❌ A fork of mprocs
- ❌ An extension of mprocs
- ❌ "mprocs with AI added"

Termin.AI **IS**:
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
- **Anthropic Claude** - AI capabilities
- **Ratatui** - Terminal UI framework
- **genai** - Multi-provider LLM client

---

**Status:** Alpha - Active Development

For questions, issues, or contributions, please visit our [GitHub repository](https://github.com/yourusername/termin.ai).
