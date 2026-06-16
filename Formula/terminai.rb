class Terminai < Formula
  desc "Interactive terminal wrapper with AI assistant"
  homepage "https://github.com/emosenkis/termin.ai"
  url "https://github.com/emosenkis/termin.ai.git",
      using:  :git,
      branch: "main"
  version "0.1.0"
  license "MIT"

  depends_on "rust" => :build

  def install
    # Build the Rust binary (only terminai, not termcap test utility)
    system "cargo", "build", "--release", "-p", "termin", "--bin", "terminai"

    # Install the binary to libexec and create a wrapper script at bin/terminai.
    libexec.install "target/release/terminai" => "terminai-unwrapped"

    (bin/"terminai").write <<~EOS
      #!/bin/bash
      exec "#{libexec}/terminai-unwrapped" "$@"
    EOS

    chmod 0755, bin/"terminai"
  end

  def caveats
    <<~EOS
      Termin.AI runs your configured CLI agent in a PTY-backed overlay.
      It does not store AI credentials or choose models itself.

      Recommended setup:
        $ terminai init-config
        $ codex login
        # or:
        $ claude auth

      Configuration file:
        ~/.config/terminai/terminai.yaml

      To use Termin.AI:
        $ terminai

      Press Ctrl+Space to open the CLI-agent overlay.

      For more information, see: https://github.com/emosenkis/termin.ai
    EOS
  end

  test do
    # Test that the binary exists and can show help
    assert_match "Interactive terminal wrapper with AI assistant",
      shell_output("#{bin}/terminai --help")
  end
end
