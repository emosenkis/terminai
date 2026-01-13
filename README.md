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

### macOS (Homebrew - Recommended)

```bash
# Add the tap and install
brew tap emosenkis/termin.ai https://github.com/emosenkis/termin.ai.git
brew install terminai
```

For other Mac installation methods, see [INSTALL_MACOS.md](INSTALL_MACOS.md).

### macOS (Installation Script)

```bash
git clone https://github.com/emosenkis/termin.ai.git
cd termin.ai
./scripts/install-macos.sh
```

### Linux / From Source

```bash
git clone https://github.com/emosenkis/termin.ai.git
cd termin.ai
cargo build --release -p termin

# Install binary
sudo cp target/release/terminai /usr/local/bin/

# Install Python agent
sudo mkdir -p /usr/local/lib/terminai
sudo cp -r python /usr/local/lib/terminai/
cd /usr/local/lib/terminai/python
uv sync --frozen
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

### Quick Start (Environment Variables)

The simplest way to get started is to set an API key environment variable:

```bash
# Choose one:
export ANTHROPIC_API_KEY="sk-ant-..."
export OPENAI_API_KEY="sk-..."
export GEMINI_API_KEY="..."
export OPENROUTER_API_KEY="sk-or-..."
```

Termin.AI will automatically detect and use the first available API key.

### Configuration File

For more control, create `~/.config/mprocs/mprocs.yaml`:

```yaml
ai:
  # Enable AI assistance (required)
  enabled: true

  # Provider: anthropic, openai, gemini, ollama, or openrouter
  provider: anthropic

  # Model name (optional - uses provider default if not specified)
  model: claude-3-5-sonnet-20241022

  # Custom endpoint URL (optional)
  # Useful for OpenRouter, Azure OpenAI, or self-hosted models
  # endpoint: https://api.custom.com/v1

  # Custom API key environment variable (optional)
  # If not specified, uses provider's default env var
  # api_key_env: MY_CUSTOM_API_KEY
```

### Supported Providers

#### Anthropic Claude

**Default Model**: `claude-3-5-sonnet-20241022`
**Environment Variable**: `ANTHROPIC_API_KEY`
**Get API Key**: https://console.anthropic.com/

```yaml
ai:
  enabled: true
  provider: anthropic
  model: claude-3-5-sonnet-20241022
```

**Available Models**:
- `claude-3-5-sonnet-20241022` (recommended - best balance)
- `claude-3-opus-20240229` (most capable)
- `claude-3-sonnet-20240229` (fast)
- `claude-3-haiku-20240307` (fastest, cheapest)

#### OpenAI

**Default Model**: `gpt-4-turbo`
**Environment Variable**: `OPENAI_API_KEY`
**Get API Key**: https://platform.openai.com/api-keys

```yaml
ai:
  enabled: true
  provider: openai
  model: gpt-4-turbo
```

**Available Models**:
- `gpt-4-turbo` (recommended)
- `gpt-4o` (multimodal)
- `gpt-4o-mini` (fast, cheap)
- `gpt-4` (older, reliable)
- `gpt-3.5-turbo` (cheapest)

#### Google Gemini

**Default Model**: `gemini-pro`
**Environment Variable**: `GEMINI_API_KEY` or `GOOGLE_API_KEY`
**Get API Key**: https://makersuite.google.com/app/apikey

```yaml
ai:
  enabled: true
  provider: gemini
  model: gemini-pro
```

**Available Models**:
- `gemini-pro` (recommended)
- `gemini-2.0-flash` (fast)
- `gemini-ultra` (most capable, when available)

#### Ollama (Local Models)

**Default Model**: `llama2`
**Environment Variable**: None (runs locally)
**Installation**: https://ollama.ai/

```yaml
ai:
  enabled: true
  provider: ollama
  model: llama3.2
```

**Note**: Requires Ollama running locally. Install models with `ollama pull <model>`.

**Popular Models**:
- `llama3.2`, `llama3`
- `codellama` (optimized for code)
- `mistral`, `mixtral`
- See full list: https://ollama.ai/library

#### OpenRouter (Multi-Provider Access)

**NEW!** OpenRouter provides access to 100+ AI models through a single API.

**Default Model**: `anthropic/claude-3-5-sonnet`
**Environment Variable**: `OPENROUTER_API_KEY`
**Get API Key**: https://openrouter.ai/keys

```yaml
ai:
  enabled: true
  provider: openrouter
  model: anthropic/claude-3-5-sonnet
  # Endpoint automatically defaults to https://openrouter.ai/api/v1
```

**Popular Models** (OpenRouter format):
- `anthropic/claude-3-5-sonnet`
- `openai/gpt-4-turbo`
- `google/gemini-pro`
- `meta-llama/llama-3-70b-instruct`
- `mistralai/mixtral-8x7b-instruct`

See all models: https://openrouter.ai/models

**Benefits**:
- Try different models without managing multiple API keys
- Access to models not directly available
- Automatic fallback if a model is unavailable
- Pay-per-use pricing across all providers

### Advanced Configuration

#### Custom Endpoints

Use custom API endpoints for Azure OpenAI, AWS Bedrock, or self-hosted models:

```yaml
ai:
  enabled: true
  provider: openai
  model: gpt-4
  endpoint: https://your-azure-endpoint.openai.azure.com/openai/deployments/your-deployment/
  api_key_env: AZURE_OPENAI_KEY
```

#### Custom API Key Environment Variables

Specify which environment variable to use for the API key:

```yaml
ai:
  enabled: true
  provider: anthropic
  api_key_env: MY_TEAM_ANTHROPIC_KEY  # Instead of default ANTHROPIC_API_KEY
```

#### Multiple Environments

Use different configs for different purposes:

```yaml
# ~/.config/mprocs/mprocs.dev.yaml (development)
ai:
  enabled: true
  provider: openai
  model: gpt-4o-mini  # Cheaper for testing

# ~/.config/mprocs/mprocs.prod.yaml (production)
ai:
  enabled: true
  provider: anthropic
  model: claude-3-opus-20240229  # Best quality
```

#### Environment Variable Fallback

If no configuration file exists, Termin.AI automatically tries these environment variables in order:

1. `ANTHROPIC_API_KEY`
2. `OPENAI_API_KEY`
3. `GOOGLE_API_KEY` or `GEMINI_API_KEY`
4. `OPENROUTER_API_KEY`

The first available API key will be used with its default provider and model.

### Configuration Examples

#### Minimal (Environment Variable Only)

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
terminai  # No config file needed!
```

#### Basic YAML Config

```yaml
ai:
  enabled: true
  provider: anthropic
```

#### Complete Configuration

```yaml
ai:
  enabled: true
  provider: openrouter
  model: anthropic/claude-3-opus
  api_key_env: OPENROUTER_API_KEY

# Future options (not yet implemented):
# shell:
#   path: /bin/zsh
#   args: ["-l"]
# privacy:
#   custom_filters: ["CUSTOM_SECRET_.*"]
```

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
