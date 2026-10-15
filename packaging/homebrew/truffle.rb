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
      sha256 "REPLACE_WITH_SHA256"
    end
    on_intel do
      url "https://github.com/Azilone/truffle/releases/download/v#{version}/truffle-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_SHA256"
    end
  end
  on_linux do
    on_arm do
      url "https://github.com/Azilone/truffle/releases/download/v#{version}/truffle-v#{version}-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "REPLACE_WITH_SHA256"
    end
    on_intel do
      url "https://github.com/Azilone/truffle/releases/download/v#{version}/truffle-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "REPLACE_WITH_SHA256"
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
