# macOS Build Support Implementation

**Date**: 2026-01-13
**Branch**: `claude/macos-build-support-0MvAw`

## Overview

This document summarizes the macOS build support and Homebrew installation infrastructure added to Termin.AI.

## What Was Added

### 1. Homebrew Formula (`Formula/terminai.rb`)

A complete Homebrew formula that:
- Builds the Rust binary using Cargo
- Installs the Python agent with UV
- Sets up proper directory structure (`#{libexec}/python/`)
- Creates a wrapper script to ensure UV is in PATH
- Includes comprehensive tests and installation caveats

**Key Features**:
- Declares dependencies: `rust` (build), `uv` (runtime), `python@3.11` (runtime)
- Uses `std_cargo_args` for standard Cargo build flags
- Installs Python agent alongside binary for proper relative path discovery
- Includes post-install caveats about API keys and configuration

### 2. macOS Installation Script (`scripts/install-macos.sh`)

A user-friendly installation script that:
- Verifies prerequisites (Rust, Python 3.11+, UV)
- Builds the Rust binary from source
- Installs to `~/.local/bin` (customizable via `INSTALL_DIR`)
- Sets up Python agent with UV
- Provides clear success messages and next steps

**Features**:
- Color-coded output (info, warn, error)
- Automatic UV installation if missing
- PATH detection and recommendations
- Customizable installation directory

### 3. Comprehensive Documentation

#### `INSTALL_MACOS.md`
Complete macOS installation guide covering:
- **3 Installation Methods**: Homebrew (recommended), installation script, manual build
- **Configuration**: API key setup, config file examples
- **Troubleshooting**: Common issues and solutions
- **Platform Notes**: Apple Silicon and Intel Mac specifics

#### `HOMEBREW_TAP.md`
Technical documentation for maintainers:
- How to set up and use the Homebrew tap
- Publishing updates and version management
- Formula testing procedures
- Directory structure and installation locations
- Future improvements (bottles, public release)

### 4. Updated Build Scripts

#### `scripts/build-unix.sh`
Updated to build `terminai` instead of `mprocs`:
- Changed package name from `mprocs` to `termin`
- Changed binary name from `mprocs` to `terminai`
- Added Python agent to release tarball
- Updated release directory structure

### 5. GitHub Actions Workflow (`.github/workflows/macos-build.yml`)

Automated CI/CD for macOS:
- Builds on macOS latest
- Runs Rust and Python tests
- Tests installation script
- Validates Homebrew formula syntax
- Uploads build artifacts
- Caches Rust dependencies for faster builds

### 6. Updated README (`README.md`)

Added prominent macOS installation section:
- Homebrew installation (primary method)
- Installation script (alternative)
- Linux/from-source instructions
- Links to detailed Mac documentation

## Installation Methods for Users

### Method 1: Homebrew (Recommended)

```bash
brew tap emosenkis/termin.ai https://github.com/emosenkis/termin.ai.git
brew install terminai
```

**Pros**:
- Automatic dependency management
- Easy updates (`brew upgrade terminai`)
- Standard macOS package manager
- Handles Python virtual environment automatically

**Cons**:
- Requires Homebrew installed
- Requires SSH access to repository

### Method 2: Installation Script

```bash
git clone https://github.com/emosenkis/termin.ai.git
cd termin.ai
./scripts/install-macos.sh
```

**Pros**:
- No Homebrew required
- Full control over installation location
- Works without SSH keys (if repo is public)

**Cons**:
- Manual updates required
- Must have prerequisites installed

### Method 3: Manual Build

```bash
git clone https://github.com/emosenkis/termin.ai.git
cd termin.ai
cargo build --release -p termin
# ... manual installation steps
```

**Pros**:
- Maximum control
- Good for development

**Cons**:
- Most manual work
- Easy to miss steps

## Technical Details

### Directory Structure (Homebrew)

On Apple Silicon:
```
/opt/homebrew/
├── bin/terminai                               # Wrapper script
├── libexec/
│   ├── terminai-unwrapped                     # Actual binary
│   └── python/                                # Python agent
│       ├── terminai_agent/
│       │   ├── __init__.py
│       │   ├── agent.py
│       │   └── ...
│       ├── pyproject.toml
│       └── .venv/                             # UV-managed venv
└── ...
```

On Intel:
- Same structure under `/usr/local/` instead

### How the Rust Binary Finds Python

From `src/llm_subprocess.rs:90-103`:

```rust
let python_dir = if let Some(dir) = config.python_dir {
  // Explicit directory (for tests)
  dir
} else {
  // Auto-detect: ../python relative to executable
  std::env::current_exe()?
    .parent()?
    .join("../python")
    .canonicalize()
    .or_else(|_| {
      // Fallback: ./python in current directory
      std::env::current_dir()?.join("python").canonicalize()
    })?
};
```

The Homebrew formula places Python at `#{libexec}/python/`, which is `../python` relative to `#{libexec}/terminai-unwrapped`.

### Python Subprocess Launch

From `src/llm_subprocess.rs:112-129`:

```rust
let mut command = Command::new("uv");
command
  .arg("run")
  .arg("python")
  .arg("-m")
  .arg("terminai_agent")
  .arg("--secret")
  .arg(&secret)
  // ... more args
  .current_dir(&python_dir)
```

This requires:
1. `uv` in PATH (ensured by wrapper script)
2. Python directory with `pyproject.toml` and `terminai_agent/`
3. UV-managed venv with dependencies installed

## Platform Compatibility

### Verified Working

✅ **Code Audit Results**:
- `src/clipboard.rs:21-27` - macOS support via `pbcopy`
- `scripts/build-unix.sh:21-41` - Darwin target handling
- `src/settings.rs:82-90` - XDG paths work on macOS
- No platform-specific blockers found

✅ **Build System**:
- Cargo builds work on Darwin targets
- Python 3.11+ available via Homebrew
- UV available via Homebrew

### Testing Checklist

Before release, test on:
- [ ] macOS 15 Sequoia (Apple Silicon)
- [ ] macOS 14 Sonoma (Apple Silicon)
- [ ] macOS 14 Sonoma (Intel)
- [ ] macOS 13 Ventura (Intel)

Test scenarios:
- [ ] Homebrew installation
- [ ] Installation script
- [ ] Manual build from source
- [ ] Launch terminai and verify no errors
- [ ] Activate AI overlay (Ctrl+Space)
- [ ] Execute a command via AI
- [ ] Check Python subprocess spawns correctly

## Future Improvements

### 1. Pre-built Bottles

Create binary bottles to speed up installation:
```ruby
bottle do
  root_url "https://github.com/emosenkis/termin.ai/releases/download/v0.1.0"
  sha256 cellar: :any_skip_relocation, arm64_sonoma: "..."
  sha256 cellar: :any_skip_relocation, ventura: "..."
end
```

### 2. Submit to homebrew-core

When ready for public release:
1. Make repository public
2. Create stable releases with version tags
3. Submit formula to `homebrew-core`
4. Follow [Homebrew contribution guide](https://docs.brew.sh/How-To-Open-a-Homebrew-Pull-Request)

### 3. Code Signing

For better macOS integration:
```bash
codesign --sign "Developer ID" --timestamp \
  --options runtime target/release/terminai
```

### 4. DMG Installer

Create a drag-and-drop installer:
```bash
create-dmg \
  --volname "Termin.AI" \
  --window-pos 200 120 \
  --window-size 800 400 \
  --icon-size 100 \
  --app-drop-link 600 185 \
  TerminAI.dmg \
  target/release/
```

## Dependencies

### Build Dependencies
- **Rust** (stable): For compiling the binary
- **Cargo**: Rust package manager

### Runtime Dependencies
- **UV**: Python package manager and virtual environment
- **Python 3.11+**: Runtime for Python agent

### Optional
- **Homebrew**: For package management method

## References

### Documentation Created
- [`Formula/terminai.rb`](Formula/terminai.rb) - Homebrew formula
- [`scripts/install-macos.sh`](scripts/install-macos.sh) - Installation script
- [`INSTALL_MACOS.md`](INSTALL_MACOS.md) - User installation guide
- [`HOMEBREW_TAP.md`](HOMEBREW_TAP.md) - Maintainer guide
- [`.github/workflows/macos-build.yml`](.github/workflows/macos-build.yml) - CI/CD workflow

### Modified Files
- [`README.md`](README.md) - Added macOS installation section
- [`scripts/build-unix.sh`](scripts/build-unix.sh) - Updated for terminai

### Resources Used
- [Homebrew Formula Cookbook](https://docs.brew.sh/Formula-Cookbook)
- [Python for Formula Authors](https://docs.brew.sh/Python-for-Formula-Authors)
- [UV Documentation](https://docs.astral.sh/uv/)
- [Homebrew Formulae - awscli](https://formulae.brew.sh/formula/awscli) - Example reference
- [Homebrew Formulae - uv](https://formulae.brew.sh/formula/uv) - UV formula reference

## Next Steps

1. **Commit Changes**:
   ```bash
   git add -A
   git commit -m "feat: add macOS build support and Homebrew formula"
   git push -u origin claude/macos-build-support-0MvAw
   ```

2. **Test on macOS**:
   - Test Homebrew installation on M1/M2/M3 Mac
   - Test installation script
   - Verify AI overlay works
   - Test Python subprocess spawns correctly

3. **Create PR**:
   - Create pull request to main branch
   - Link to this documentation
   - Request testing on various macOS versions

4. **After Merge**:
   - Tag release: `git tag v0.1.0 && git push origin v0.1.0`
   - Update documentation with actual installation commands
   - Consider creating pre-built bottles

## Questions or Issues?

For macOS-specific issues:
- Check [INSTALL_MACOS.md](INSTALL_MACOS.md) troubleshooting section
- Open GitHub issue with `macOS` label

For Homebrew-specific issues:
- Check [HOMEBREW_TAP.md](HOMEBREW_TAP.md)
- Test formula with `brew install --verbose`

---

**Implementation by**: Claude Code
**Date**: 2026-01-13
**Status**: Ready for testing
