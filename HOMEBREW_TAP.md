# Setting Up Homebrew Tap for Termin.AI

This document explains how to set up and use the Homebrew tap for Termin.AI.

## What is a Homebrew Tap?

A Homebrew tap is a third-party repository that allows users to install packages not included in Homebrew's main repository. For Termin.AI, the tap allows users to install directly from the project repository.

## For Users: Installing from the Tap

### Prerequisites

1. **Homebrew** installed ([install here](https://brew.sh/))
2. **SSH access** to the repository (configured in your `~/.ssh/config` and GitHub account)

### Installation

```bash
# Add the tap
brew tap emosenkis/termin.ai https://github.com/emosenkis/termin.ai.git

# Install Termin.AI
brew install terminai
```

### What This Does

1. Clones the repository to `$(brew --repository)/Library/Taps/emosenkis/homebrew-termin.ai`
2. Finds the formula at `Formula/terminai.rb`
3. Builds the Rust binary using Cargo
4. Installs the Python agent and dependencies using UV
5. Sets up the proper directory structure

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

## For Maintainers: Publishing Updates

### Directory Structure

The tap expects this structure:

```
termin.ai/
├── Formula/
│   └── terminai.rb          # Homebrew formula
├── src/
│   └── Cargo.toml           # Rust project
├── python/
│   ├── terminai_agent/      # Python package
│   └── pyproject.toml       # Python dependencies
└── ...
```

### Updating the Formula

The formula (`Formula/terminai.rb`) automatically uses the latest code from the repository. When you push changes:

1. **Code Changes**: Push to main branch
2. **Users Update**: Run `brew update && brew upgrade terminai`
3. **Formula Changes**: Edit `Formula/terminai.rb` and commit

### Version Updates

To release a new version:

1. Update version in `src/Cargo.toml`
2. Update version in `python/pyproject.toml`
3. Update version in `Formula/terminai.rb`
4. Commit and push:
   ```bash
   git add src/Cargo.toml python/pyproject.toml Formula/terminai.rb
   git commit -m "chore: bump version to X.Y.Z"
   git push origin main
   ```
5. Tag the release:
   ```bash
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```

### Testing the Formula

Before releasing, test the formula locally:

```bash
# Test syntax
brew audit --strict --online Formula/terminai.rb

# Test installation (dry run)
brew install --build-from-source Formula/terminai.rb --verbose

# Test actual installation
brew uninstall terminai
brew install --build-from-source Formula/terminai.rb

# Test the installed binary
terminai --help
```

### Formula Best Practices

1. **Dependencies**: Keep `depends_on` up to date
   ```ruby
   depends_on "rust" => :build  # Build-time only
   depends_on "uv"              # Runtime dependency
   depends_on "python@3.11"     # Runtime dependency
   ```

2. **Testing**: Update the `test do` block when adding features
   ```ruby
   test do
     assert_match "Interactive terminal", shell_output("#{bin}/terminai --help")
   end
   ```

3. **Caveats**: Keep installation instructions current
   ```ruby
   def caveats
     <<~EOS
       Important usage information here
     EOS
   end
   ```

## How the Formula Works

### Build Process

1. **Rust Binary**:
   ```ruby
   system "cargo", "install", *std_cargo_args(path: "src")
   ```
   Builds and installs the Rust binary to `#{bin}/terminai`

2. **Python Agent**:
   ```ruby
   python_dir = libexec/"python"
   cp_r "python/terminai_agent", python_dir/"terminai_agent"
   cp "python/pyproject.toml", python_dir/"pyproject.toml"
   ```
   Copies Python files to `#{libexec}/python/`

3. **Dependencies**:
   ```ruby
   cd python_dir do
     system "uv", "sync", "--frozen"
   end
   ```
   Installs Python dependencies using UV

4. **Wrapper Script**:
   ```ruby
   (bin/"terminai").write <<~EOS
     #!/bin/bash
     export PATH="#{HOMEBREW_PREFIX}/bin:$PATH"
     exec "#{libexec}/terminai-unwrapped" "$@"
   EOS
   ```
   Creates a wrapper to ensure UV is in PATH

### Installation Locations

On Apple Silicon (M1/M2/M3):
- Binary: `/opt/homebrew/bin/terminai`
- Python: `/opt/homebrew/libexec/python/`
- Wrapper: `/opt/homebrew/libexec/terminai-unwrapped`

On Intel Macs:
- Binary: `/usr/local/bin/terminai`
- Python: `/usr/local/libexec/python/`
- Wrapper: `/usr/local/libexec/terminai-unwrapped`

## Private Repository Considerations

Since this tap points to a private repository:

1. **SSH Access Required**: Users must have SSH keys configured for GitHub
2. **Authentication**: Homebrew uses the system's SSH configuration
3. **Access Control**: Only users with repository access can install

### For Users Without SSH Access

Provide alternative installation methods in [INSTALL_MACOS.md](INSTALL_MACOS.md):
- Direct installation script
- Manual build from source
- Release tarballs (if you create releases)

## Troubleshooting

### "fatal: could not read Username"

User doesn't have SSH access configured. They should:
1. Set up SSH keys: https://docs.github.com/en/authentication/connecting-to-github-with-ssh
2. Test access: `ssh -T git@github.com`
3. Try again: `brew tap emosenkis/termin.ai`

### "Error: No available formula"

The formula file is missing or misnamed. Check:
```bash
ls -la Formula/terminai.rb
```

### Build failures

Check the build logs:
```bash
brew install terminai --verbose
```

Common issues:
- Missing Rust installation
- Python version too old (< 3.11)
- UV not available
- Network issues during dependency download

## Future Improvements

### Public Release (When Ready)

To make the tap public:

1. Make the repository public
2. Submit to `homebrew-core`:
   ```bash
   brew create https://github.com/emosenkis/termin.ai/archive/vX.Y.Z.tar.gz
   ```
3. Follow the [Homebrew contribution guide](https://docs.brew.sh/How-To-Open-a-Homebrew-Pull-Request)

### Pre-built Bottles

To speed up installation, create pre-built binaries:

1. Build on multiple architectures:
   - macOS ARM64 (Apple Silicon)
   - macOS x86_64 (Intel)

2. Upload to GitHub Releases

3. Update formula with bottle URLs:
   ```ruby
   bottle do
     root_url "https://github.com/emosenkis/termin.ai/releases/download/vX.Y.Z"
     sha256 cellar: :any_skip_relocation, arm64_sonoma: "..."
     sha256 cellar: :any_skip_relocation, x86_64_sonoma: "..."
   end
   ```

### Continuous Integration

The GitHub Actions workflow at `.github/workflows/macos-build.yml` tests:
- Building on macOS
- Running tests
- Installation script
- Formula syntax

## Resources

- [Homebrew Formula Cookbook](https://docs.brew.sh/Formula-Cookbook)
- [Python for Formula Authors](https://docs.brew.sh/Python-for-Formula-Authors)
- [Homebrew Taps](https://docs.brew.sh/Taps)
- [Creating Homebrew Formulas](https://docs.brew.sh/Adding-Software-to-Homebrew)

## Questions?

For tap-specific issues:
- Open an issue: https://github.com/emosenkis/termin.ai/issues
- Tag with `homebrew` label

For Homebrew itself:
- Homebrew Discussions: https://github.com/Homebrew/brew/discussions
- Homebrew Docs: https://docs.brew.sh/
