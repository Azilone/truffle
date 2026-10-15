# Homebrew formula for Truffle. Goes in a tap (github.com/Azilone/homebrew-tap, Formula/truffle.rb); fill in the
# version and the SHA-256s from a release's *.sha256 files.
class Truffle < Formula
  desc "Find any file by name, content or meaning. Instantly, offline"
  homepage "https://github.com/Azilone/truffle"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/Azilone/truffle/releases/download/v#{version}/truffle-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "c64353d82a94e99ed6e87dee045791438f3562f3c5649ff6c85829cbbb4dec6b"
    end
    on_intel do
      url "https://github.com/Azilone/truffle/releases/download/v#{version}/truffle-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "3593919cf3f54135277d8a0241489d878c1ee047d3f776d7b0a04906e73e4945"
    end
  end
  on_linux do
    on_arm do
      url "https://github.com/Azilone/truffle/releases/download/v#{version}/truffle-v#{version}-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "45a466210ec5fde08c9027da1f44daa72874ea2adddc4262d3ed6c484c4e0d1e"
    end
    on_intel do
      url "https://github.com/Azilone/truffle/releases/download/v#{version}/truffle-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "71050a80823b2c710dc4a6b78b9c551fc2ae728e4287b388e3fae488be3c3fd6"
    end
  end

  def install
    bin.install "truffle"
  end

  def caveats
    <<~EOS
      Start the daemon at login and fetch the model (865 MB, once):
        truffle install --login
        truffle model download
        truffle meaning folders add ~/Documents ~/Downloads ~/Desktop
      On macOS, give ~/.local/bin/truffle Full Disk Access in System Settings › Privacy & Security.
    EOS
  end

  test do
    assert_match "llama.cpp", shell_output("#{bin}/truffle meaning version")
  end
end
