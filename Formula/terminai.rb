class Terminai < Formula
  desc "Interactive terminal wrapper with AI assistant"
  homepage "https://github.com/emosenkis/termin.ai"

  if OS.mac?
    if Hardware::CPU.arm?
      url "https://github.com/emosenkis/termin.ai/releases/download/v0.1.0/terminai-0.1.0-darwin-aarch64.tar.gz"
    else
      url "https://github.com/emosenkis/termin.ai/releases/download/v0.1.0/terminai-0.1.0-darwin-x86_64.tar.gz"
    end
  elsif OS.linux?
    odie "Termin.AI currently ships Linux binaries for x86_64 only" unless Hardware::CPU.intel?

    url "https://github.com/emosenkis/termin.ai/releases/download/v0.1.0/terminai-0.1.0-linux-x86_64-musl.tar.gz"
  end

  sha256 :no_check
  license "MIT"

  def install
    bin.install "terminai"
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
    assert_match "Interactive terminal wrapper with AI assistant",
      shell_output("#{bin}/terminai --help")
  end
end
