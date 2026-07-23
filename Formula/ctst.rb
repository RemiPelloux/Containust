# Homebrew formula for the ctst CLI (P10.14).
#
# Install directly from this repository:
#   brew install --formula ./Formula/ctst.rb
#
# The `sha256` values below must be refreshed for every release from the
# published `SHA256SUMS` asset (see .github/workflows/release.yml). A CI
# step to automate the bump is tracked in docs/PACKAGING.md.
class Ctst < Formula
  desc "Containust - lightweight, daemonless container runtime"
  homepage "https://github.com/RemiPelloux/Containust"
  version "1.1.0"
  license "MIT OR Apache-2.0"

  on_macos do
    on_arm do
      url "https://github.com/RemiPelloux/Containust/releases/download/v#{version}/ctst-aarch64-apple-darwin.tar.gz"
      sha256 :no_check # refreshed from SHA256SUMS at release time
    end
    on_intel do
      url "https://github.com/RemiPelloux/Containust/releases/download/v#{version}/ctst-x86_64-apple-darwin.tar.gz"
      sha256 :no_check # refreshed from SHA256SUMS at release time
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/RemiPelloux/Containust/releases/download/v#{version}/ctst-aarch64-unknown-linux-gnu.tar.gz"
      sha256 :no_check # refreshed from SHA256SUMS at release time
    end
    on_intel do
      url "https://github.com/RemiPelloux/Containust/releases/download/v#{version}/ctst-x86_64-unknown-linux-gnu.tar.gz"
      sha256 :no_check # refreshed from SHA256SUMS at release time
    end
  end

  depends_on "qemu" if OS.mac?

  def install
    bin.install "ctst"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/ctst --version")
  end
end
