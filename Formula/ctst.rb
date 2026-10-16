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
  version "1.4.0"
  license :cannot_represent # Containust Commercial License — see LICENSE

  on_macos do
    on_arm do
      url "https://github.com/RemiPelloux/Containust/releases/download/v#{version}/ctst-aarch64-apple-darwin.tar.gz"
      sha256 "cfda5a6abfb9d57d7ac1580feeb3cb9f47783dde8bddf2f14f6b796c4ecdd106"
    end
    on_intel do
      url "https://github.com/RemiPelloux/Containust/releases/download/v#{version}/ctst-x86_64-apple-darwin.tar.gz"
      sha256 "edc8e4954377a13d55f55e5892d5231c9b8635b0acd4a27eb31a525743d15813"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/RemiPelloux/Containust/releases/download/v#{version}/ctst-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "a8b40524e60da6023c2a6e935b0704b302afb3762c94b69441ea8b675af4ba86"
    end
    on_intel do
      url "https://github.com/RemiPelloux/Containust/releases/download/v#{version}/ctst-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "037e044af4bd882dd82037d74762e91b67f9289e060c350980d8f4cb3eade0e9"
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
