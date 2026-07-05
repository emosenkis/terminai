class Terminai < Formula
  desc "Interactive terminal wrapper with AI assistant"
  homepage "https://github.com/emosenkis/terminai"

  if OS.mac?
    if Hardware::CPU.arm?
      url "https://github.com/emosenkis/terminai/releases/download/v0.1.0/terminai-0.1.0-darwin-aarch64.tar.gz"
      sha256 "ea07f74779bd2c94f7dadb0b54dcbe13e4ba7a91d22a482c2d44099afdcb23dd"
    else
      url "https://github.com/emosenkis/terminai/releases/download/v0.1.0/terminai-0.1.0-darwin-x86_64.tar.gz"
      sha256 "a240b7f950cb07b66a062ce590ba8d0ccbbc704ad14edf974ffe4d420942ead4"
    end
  elsif OS.linux?
    odie "Terminai currently ships Linux binaries for x86_64 only" unless Hardware::CPU.intel?

    url "https://github.com/emosenkis/terminai/releases/download/v0.1.0/terminai-0.1.0-linux-x86_64-musl.tar.gz"
    sha256 "13208c59faa0c4dd911cc0bb21def79fd47c84200ab1cf426c54be4ad03deb9c"
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
