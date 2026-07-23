#!/usr/bin/env bash
# Bump Formula/ctst.rb and packaging/winget from a release SHA256SUMS (P11.7 / P11.8).
#
# Usage:
#   ./scripts/bump_packaging.sh <version> <path-to-SHA256SUMS>
# Example:
#   ./scripts/bump_packaging.sh 1.2.0 ./SHA256SUMS
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 <version> <SHA256SUMS>" >&2
  exit 2
fi

VERSION="${1#v}"
SUMS="$2"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FORMULA="$ROOT/Formula/ctst.rb"
WINGET="$ROOT/packaging/winget/Containust.ctst.yaml"

if [[ ! -f "$SUMS" ]]; then
  echo "error: SHA256SUMS not found: $SUMS" >&2
  exit 1
fi

hash_for() {
  local artifact="$1"
  # Lines look like: <sha256>  <optional path prefix>ctst-<target>.tar.gz
  local line
  line="$(grep -E "[^ ]${artifact}$| ${artifact}$" "$SUMS" | head -n1 || true)"
  if [[ -z "$line" ]]; then
    echo "error: no checksum for ${artifact} in ${SUMS}" >&2
    exit 1
  fi
  awk '{print $1}' <<<"$line"
}

SHA_AARCH64_DARWIN="$(hash_for "ctst-aarch64-apple-darwin.tar.gz")"
SHA_X86_64_DARWIN="$(hash_for "ctst-x86_64-apple-darwin.tar.gz")"
SHA_AARCH64_LINUX="$(hash_for "ctst-aarch64-unknown-linux-gnu.tar.gz")"
SHA_X86_64_LINUX="$(hash_for "ctst-x86_64-unknown-linux-gnu.tar.gz")"
SHA_WIN_X64="$(hash_for "ctst-x86_64-pc-windows-msvc.zip")"

python3 - "$FORMULA" "$VERSION" \
  "$SHA_AARCH64_DARWIN" "$SHA_X86_64_DARWIN" \
  "$SHA_AARCH64_LINUX" "$SHA_X86_64_LINUX" <<'PY'
import pathlib, re, sys
path, version, aarch64_d, x64_d, aarch64_l, x64_l = sys.argv[1:7]
text = pathlib.Path(path).read_text()
text = re.sub(r'version "[\d.]+"', f'version "{version}"', text, count=1)

def replace_block(src: str, marker: str, sha: str) -> str:
    # Replace the sha256 line that follows a url containing marker.
    pattern = (
        rf'(url "[^"]*{re.escape(marker)}[^"]*"\n\s*)sha256 [^\n]+'
    )
    repl = rf'\1sha256 "{sha}"'
    out, n = re.subn(pattern, repl, src, count=1)
    if n != 1:
        raise SystemExit(f"failed to patch sha for {marker}")
    return out

text = replace_block(text, "aarch64-apple-darwin", aarch64_d)
text = replace_block(text, "x86_64-apple-darwin", x64_d)
text = replace_block(text, "aarch64-unknown-linux-gnu", aarch64_l)
text = replace_block(text, "x86_64-unknown-linux-gnu", x64_l)
pathlib.Path(path).write_text(text)
print(f"updated {path} → {version}")
PY

python3 - "$WINGET" "$VERSION" "$SHA_WIN_X64" <<'PY'
import pathlib, re, sys
path, version, sha = sys.argv[1:4]
text = pathlib.Path(path).read_text()
text = re.sub(r"(PackageVersion:\s*)[\d.]+", rf"\g<1>{version}", text, count=1)
text = re.sub(
    r"(InstallerUrl:\s*)https://github.com/RemiPelloux/Containust/releases/download/v[\d.]+/",
    rf"\g<1>https://github.com/RemiPelloux/Containust/releases/download/v{version}/",
    text,
    count=1,
)
text = re.sub(
    r"(InstallerSha256:\s*)\S+",
    rf"\g<1>{sha}",
    text,
    count=1,
)
pathlib.Path(path).write_text(text)
print(f"updated {path} → {version} sha={sha[:12]}…")
PY

echo "packaging bump complete for v${VERSION}"
