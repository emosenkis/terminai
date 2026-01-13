class Terminai < Formula
  desc "Interactive terminal wrapper with AI assistant"
  homepage "https://github.com/emosenkis/termin.ai"
  url "git@github.com:emosenkis/termin.ai.git",
      branch: "main"
  version "0.1.0"
  license "MIT"

  depends_on "rust" => :build
  depends_on "uv"
  depends_on "python@3.11"

  def install
    # Build the Rust binary (only terminai, not termcap test utility)
    system "cargo", "install", "--bin", "terminai", *std_cargo_args(path: "src")

    # Install the Python agent alongside the binary
    # The Rust binary expects to find the Python project at ../python relative to itself
    python_dir = libexec/"python"
    python_dir.mkpath

    # Copy Python project files
    cp_r "python/terminai_agent", python_dir/"terminai_agent"
    cp "python/pyproject.toml", python_dir/"pyproject.toml"
    cp "python/uv.lock", python_dir/"uv.lock"
    cp "python/README.md", python_dir/"README.md" if File.exist?("python/README.md")

    # Sync Python dependencies using uv
    # This creates a .venv in the python directory
    cd python_dir do
      system "uv", "sync", "--frozen"
    end

    # Create a wrapper script that ensures UV is in PATH and the Python agent can be found
    (bin/"terminai-wrapped").write <<~EOS
      #!/bin/bash
      # Wrapper for terminai that ensures proper environment
      export PATH="#{HOMEBREW_PREFIX}/bin:$PATH"
      exec "#{bin}/terminai" "$@"
    EOS

    chmod 0755, bin/"terminai-wrapped"

    # Rename the wrapper to be the main executable
    mv bin/"terminai", libexec/"terminai-unwrapped"
    mv bin/"terminai-wrapped", bin/"terminai"

    # Update the wrapper to call the unwrapped version
    (bin/"terminai").write <<~EOS
      #!/bin/bash
      # Wrapper for terminai that ensures proper environment
      export PATH="#{HOMEBREW_PREFIX}/bin:$PATH"
      exec "#{libexec}/terminai-unwrapped" "$@"
    EOS

    chmod 0755, bin/"terminai"
  end

  def caveats
    <<~EOS
      Termin.AI requires an API key from a supported AI provider:
        - Anthropic Claude: export ANTHROPIC_API_KEY="..."
        - OpenAI: export OPENAI_API_KEY="..."
        - Google Gemini: export GEMINI_API_KEY="..."
        - OpenRouter: export OPENROUTER_API_KEY="..."
        - Ollama: Install from https://ollama.ai/ (no API key needed)

      Configuration file (optional): ~/.config/terminai/config.yaml

      To use Termin.AI:
        $ terminai

      Press Ctrl+Space to activate the AI assistant overlay.

      For more information, see: https://github.com/emosenkis/termin.ai
    EOS
  end

  test do
    # Test that the binary exists and can show help
    assert_match "Interactive terminal wrapper with AI assistant",
      shell_output("#{bin}/terminai --help")

    # Verify Python agent is installed
    assert_predicate libexec/"python/terminai_agent/__init__.py", :exist?
    assert_predicate libexec/"python/pyproject.toml", :exist?
    assert_predicate libexec/"python/uv.lock", :exist?

    # Verify UV can find the Python project
    cd libexec/"python" do
      system "uv", "sync", "--frozen"
    end
  end
end
