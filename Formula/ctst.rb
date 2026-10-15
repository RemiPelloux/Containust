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
  version "1.3.0"
  license :cannot_represent # Containust Commercial License — see LICENSE

  on_macos do
    on_arm do
      url "https://github.com/RemiPelloux/Containust/releases/download/v#{version}/ctst-aarch64-apple-darwin.tar.gz"
      sha256 "9c845dc38e2ac02525d994a3f51584814a59dcbb57fde52dc27c33624fea5575"
    end
    on_intel do
      url "https://github.com/RemiPelloux/Containust/releases/download/v#{version}/ctst-x86_64-apple-darwin.tar.gz"
      sha256 "67e83407b28b215fff7962a4c9ef3780fdbc8cc41f5d298792ad23877c1b358d"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/RemiPelloux/Containust/releases/download/v#{version}/ctst-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "e5b6829b6ee7fbea9bff281fc18e07792c0bd25e3fb78684be1e09c6a31e6ec2"
    end
    on_intel do
      url "https://github.com/RemiPelloux/Containust/releases/download/v#{version}/ctst-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "c3b706da5b7f79365db2d026b87df13fe6cf8c6f74d1130d1c96601b2e0d4ab6"
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
