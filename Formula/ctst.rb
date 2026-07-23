# Homebrew formula for the ctst CLI (P10.14 / P11.7).
#
# Dedicated tap (after one-time bootstrap — see packaging/homebrew-tap/README.md):
#   brew tap RemiPelloux/containust
#   brew install ctst
#
# From this repository:
#   brew install --formula ./Formula/ctst.rb
#
# On every v* release, scripts/bump_packaging.sh refreshes version + sha256
# from SHA256SUMS (release.yml → packaging-bump job).
class Ctst < Formula
  desc "Containust - lightweight, daemonless container runtime"
  homepage "https://github.com/RemiPelloux/Containust"
  version "1.2.0"
  license :cannot_represent # Containust Commercial License — see LICENSE

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
