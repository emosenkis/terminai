class Terminai < Formula
  desc "Interactive terminal wrapper with AI assistant"
  homepage "https://github.com/emosenkis/terminai"

  if OS.mac?
    if Hardware::CPU.arm?
      url "https://github.com/emosenkis/terminai/releases/download/v0.1.0/terminai-0.1.0-darwin-aarch64.tar.gz"
      sha256 "ce9bce239544c12b05190a65d320e5ecb1a63ee821d015737d06341e9096dd2a"
    else
      url "https://github.com/emosenkis/terminai/releases/download/v0.1.0/terminai-0.1.0-darwin-x86_64.tar.gz"
      sha256 "dac25e482375fc785a15b82d195b5849400a0d4adc7cc68dc9c9f30417259585"
    end
  elsif OS.linux?
    odie "Terminai currently ships Linux binaries for x86_64 only" unless Hardware::CPU.intel?

    url "https://github.com/emosenkis/terminai/releases/download/v0.1.0/terminai-0.1.0-linux-x86_64-musl.tar.gz"
    sha256 "05ea214041cd3cabe521feea9aeffb751d796a0d62f2f007e36943416db8ef39"
  end

  license "MIT"

  def install
    bin.install "terminai"
  end

  def post_install
    system bin/"terminai", "init-config"
  end

  def caveats
    <<~EOS
      Terminai runs your configured CLI agent in a PTY-backed overlay.
      It does not store AI credentials or choose models itself.

      The default config has been initialized at:
        ~/.config/terminai/terminai.yaml

      Next, authenticate your chosen CLI agent:
        $ codex login
        # or:
        $ claude auth

      To use Terminai:
        $ terminai

      Press Ctrl+Space to open the CLI-agent overlay.

      For more information, see: https://github.com/emosenkis/terminai
    EOS
  end

  test do
    assert_match "Interactive terminal wrapper with AI assistant",
      shell_output("#{bin}/terminai --help")
  end
end
