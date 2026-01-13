# Installing Termin.AI on Mac OS

This guide covers multiple ways to install Termin.AI on Mac OS.

## Option 1: Homebrew (Recommended)

### Prerequisites

- Homebrew installed ([install here](https://brew.sh/))
- Git configured with SSH access to the repository

### Install from Homebrew Tap

```bash
# Add the tap
brew tap emosenkis/termin.ai https://github.com/emosenkis/termin.ai.git

# Install Termin.AI
brew install terminai
```

This will:
- Build the Rust binary
- Install the Python agent
- Set up all dependencies (UV, Python 3.11+)
- Create a wrapper script to ensure proper environment

### Updating

```bash
brew update
brew upgrade terminai
```

### Uninstalling

```bash
brew uninstall terminai
brew untap emosenkis/termin.ai
```

## Option 2: Direct Installation Script

If you prefer not to use Homebrew, use the provided installation script:

```bash
# Clone the repository
git clone https://github.com/emosenkis/termin.ai.git
cd termin.ai

# Run the installation script
./scripts/install-macos.sh
```

The script will:
- Check for required dependencies (Rust, Python 3.11+, uv)
- Build the Rust binary
- Install to `~/.local/bin/terminai` (customizable via `INSTALL_DIR`)
- Set up the Python agent

### Custom Installation Directory

```bash
INSTALL_DIR=/usr/local ./scripts/install-macos.sh
```

## Option 3: Manual Build from Source

### Prerequisites

1. **Rust** (latest stable)
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Python 3.11+**
   ```bash
   brew install python@3.11
   ```

3. **UV** (Python package manager)
   ```bash
   brew install uv
   ```

### Build and Install

```bash
# Clone the repository
git clone https://github.com/emosenkis/termin.ai.git
cd termin.ai

# Build the Rust binary
cargo build --release -p termin

# Install the binary
sudo cp target/release/terminai /usr/local/bin/

# Set up Python agent
mkdir -p /usr/local/lib/terminai
cp -r python /usr/local/lib/terminai/

# Install Python dependencies
cd /usr/local/lib/terminai/python
uv sync --frozen
```

## Configuration

### API Keys

Termin.AI requires an API key from a supported provider. Set one of these environment variables:

```bash
# Anthropic Claude (recommended)
export ANTHROPIC_API_KEY="sk-ant-..."

# OpenAI GPT-4
export OPENAI_API_KEY="sk-..."

# Google Gemini
export GEMINI_API_KEY="..."

# OpenRouter (multi-provider)
export OPENROUTER_API_KEY="sk-or-..."

# Ollama (local, no key needed)
# Install from https://ollama.ai/
```

Add to your `~/.zshrc` or `~/.bashrc` to persist:

```bash
echo 'export ANTHROPIC_API_KEY="sk-ant-..."' >> ~/.zshrc
source ~/.zshrc
```

### Configuration File (Optional)

Create `~/.config/terminai/config.yaml`:

```yaml
ai:
  enabled: true
  provider: anthropic  # or: openai, gemini, ollama, openrouter
  model: claude-3-5-sonnet-20241022  # optional
```

See the [main README](README.md#configuration) for full configuration options.

## Usage

Launch Termin.AI:

```bash
terminai
```

- Use your terminal normally
- Press `Ctrl+Space` to activate the AI assistant overlay
- Ask questions or request commands
- Press `Esc` to close the overlay

## Troubleshooting

### "terminai: command not found"

**Homebrew Installation:**
```bash
# Check if installed
brew list terminai

# If installed, ensure Homebrew bin is in PATH
echo 'export PATH="/opt/homebrew/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

**Script Installation:**
```bash
# Check if ~/.local/bin is in PATH
echo $PATH | grep "$HOME/.local/bin"

# If not, add it
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

### "Failed to spawn Python subprocess"

This means the Python agent can't be found. Check:

```bash
# For Homebrew installation
ls -la $(brew --prefix)/libexec/python/terminai_agent/

# For script installation
ls -la ~/.local/lib/terminai/python/terminai_agent/
```

If missing, reinstall using one of the methods above.

### "uv: command not found"

Install uv:

```bash
brew install uv
```

### Python version issues

Termin.AI requires Python 3.11+:

```bash
# Check version
python3 --version

# Install if needed
brew install python@3.11
```

### API key not detected

Make sure your API key is exported in the current shell:

```bash
# Check if set
echo $ANTHROPIC_API_KEY

# Set temporarily
export ANTHROPIC_API_KEY="sk-ant-..."

# Set permanently
echo 'export ANTHROPIC_API_KEY="sk-ant-..."' >> ~/.zshrc
source ~/.zshrc
```

### Build failures

1. **Cargo errors:**
   ```bash
   # Update Rust
   rustup update stable

   # Clean and rebuild
   cargo clean
   cargo build --release -p termin
   ```

2. **Python dependency errors:**
   ```bash
   # Update uv
   brew upgrade uv

   # Clear uv cache
   uv cache clean

   # Reinstall dependencies
   cd /path/to/python/directory
   uv sync --frozen
   ```

## Verifying Installation

```bash
# Check binary
which terminai
terminai --version

# Check Python agent (Homebrew)
ls -la $(brew --prefix)/libexec/python/terminai_agent/

# Check Python agent (script install)
ls -la ~/.local/lib/terminai/python/terminai_agent/

# Test UV can find dependencies
cd /path/to/python/directory
uv run python -c "import terminai_agent; print('OK')"
```

## Development Setup

If you're developing Termin.AI:

```bash
# Clone the repository
git clone https://github.com/emosenkis/termin.ai.git
cd termin.ai

# Build in debug mode
cargo build -p termin

# Run from source
./target/debug/terminai

# The Python agent will be auto-detected at ./python/
```

## Platform-Specific Notes

### Apple Silicon (M1/M2/M3)

All installation methods work on Apple Silicon. Homebrew will automatically use the ARM64 architecture.

### Intel Macs

All installation methods work on Intel Macs. No special configuration needed.

### macOS Versions

Termin.AI is tested on:
- macOS 15 Sequoia
- macOS 14 Sonoma
- macOS 13 Ventura

Older versions may work but are not officially supported.

## Support

For issues, questions, or contributions:
- GitHub Issues: https://github.com/emosenkis/termin.ai/issues
- Documentation: [README.md](README.md)
- Project Instructions: [CLAUDE.md](CLAUDE.md)

## See Also

- [Main README](README.md) - Full project documentation
- [Configuration Guide](README.md#configuration) - Detailed configuration options
- [Contributing Guide](CLAUDE.md) - Guidelines for contributors
