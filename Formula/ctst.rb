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
  version "1.3.1"
  license :cannot_represent # Containust Commercial License — see LICENSE

  on_macos do
    on_arm do
      url "https://github.com/RemiPelloux/Containust/releases/download/v#{version}/ctst-aarch64-apple-darwin.tar.gz"
      sha256 "03d023df0ac2046609f06074f462276083b89b8e4bf9f25264d840209031b30d"
    end
    on_intel do
      url "https://github.com/RemiPelloux/Containust/releases/download/v#{version}/ctst-x86_64-apple-darwin.tar.gz"
      sha256 "d7359390769271687251fd05eb5c52b975a4276e2cf60fa2f58eab77c1ff483b"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/RemiPelloux/Containust/releases/download/v#{version}/ctst-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "7c3266aa54efcd0d3d140e5485f39ed3943dd63826c96f066b441d925f49c80f"
    end
    on_intel do
      url "https://github.com/RemiPelloux/Containust/releases/download/v#{version}/ctst-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "8e4dd155d8da22914b78026163a0b9dd45f3e00f689ac30213cf419b26dd3622"
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
