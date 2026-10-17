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
  version "1.5.0"
  license :cannot_represent # Containust Commercial License — see LICENSE

  on_macos do
    on_arm do
      url "https://github.com/RemiPelloux/Containust/releases/download/v#{version}/ctst-aarch64-apple-darwin.tar.gz"
      sha256 "710e23e031b809a442ddd14b83d7c95e6e9ebdae3504ab5dcaffb4c4ea197304"
    end
    on_intel do
      url "https://github.com/RemiPelloux/Containust/releases/download/v#{version}/ctst-x86_64-apple-darwin.tar.gz"
      sha256 "1e29b42f3e6d6f3b7a5b5341ab10e5edf13466330f54e38581be03079269c82d"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/RemiPelloux/Containust/releases/download/v#{version}/ctst-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "ca5ca8762ce717e26345f3600262f8d178876d023089b8706306b2e74b1a1209"
    end
    on_intel do
      url "https://github.com/RemiPelloux/Containust/releases/download/v#{version}/ctst-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "b26aba7838cc367e3234d7cc78d6643b432cf2e81c25de66abf1605bd1b1ef0d"
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
