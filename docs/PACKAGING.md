# Packaging Channels

Installation paths beyond `cargo install`. Updated for Sprint 10
(P10.14–P10.17). Each channel is either implemented or explicitly deferred
with an owner.

| Channel | Status | Owner | Notes |
|---|---|---|---|
| Source / `cargo install --path crates/containust-cli` | **Supported** | maintainers | Documented in README and CLI_REFERENCE |
| GitHub Release binaries (`v*` tags) | **Supported** | maintainers | SHA-256 checksums + cosign-signed `SHA256SUMS` via `.github/workflows/release.yml` |
| Homebrew formula | **Supported** | maintainers | In-tree `Formula/ctst.rb`; dedicated tap `RemiPelloux/containust` + automated sha bump on `v*` (P11.7 — see `packaging/homebrew-tap/README.md`) |
| Debian (`.deb`) | **Supported** | maintainers | Built by the `linux-packages` release job with nfpm (`packaging/nfpm.yaml`) |
| RPM | **Supported** | maintainers | Same nfpm config as Debian |
| Windows (winget) | **Supported (submit)** | maintainers | Manifest auto-bumped on release (`scripts/bump_packaging.sh`); submit to winget-pkgs per `packaging/winget/README.md` (P11.8) |
| Windows installer (MSI) | **Deferred** | maintainers | Zip + winget cover Windows installs; MSI only if enterprise demand appears |

## Install from packages

```bash
# Debian / Ubuntu
curl -LO https://github.com/RemiPelloux/Containust/releases/download/vX.Y.Z/ctst_X.Y.Z_amd64.deb
sudo dpkg -i ctst_X.Y.Z_amd64.deb

# Fedora / RHEL
sudo rpm -i ctst-X.Y.Z-1.amd64.rpm

# Homebrew tap (macOS / Linux) — after tap bootstrap
brew tap RemiPelloux/containust && brew install ctst

# Or from a clone of this repo
brew install --formula ./Formula/ctst.rb
```

### Release packaging bump (P11.7 / P11.8)

On every `v*` tag, `.github/workflows/release.yml` runs `packaging-bump`:

1. `./scripts/bump_packaging.sh <version> SHA256SUMS` refreshes formula sha256 + winget SHA
2. Opens a PR on Containust with those changes
3. If `HOMEBREW_TAP_TOKEN` is set, pushes `Formula/ctst.rb` to
   `RemiPelloux/homebrew-containust` (see `packaging/homebrew-tap/README.md`)

## Current recommended install

```bash
# From a release tag (verify checksum)
curl -LO https://github.com/RemiPelloux/Containust/releases/download/vX.Y.Z/ctst-<target>.tar.gz
curl -LO https://github.com/RemiPelloux/Containust/releases/download/vX.Y.Z/ctst-<target>.tar.gz.sha256
sha256sum -c ctst-<target>.tar.gz.sha256
tar xzf ctst-<target>.tar.gz
sudo install -m 755 ctst /usr/local/bin/ctst
ctst --version
```

## Signing

The aggregated `SHA256SUMS` release asset is signed keylessly with cosign
(Sigstore OIDC, no long-lived secrets) — verify with the procedure in
[`RUNBOOKS.md`](RUNBOOKS.md#verify-a-release-download). Apple notarization and
Windows Authenticode remain deferred (owner: maintainers) until a paid signing
identity is provisioned; SHA-256 verification is mandatory regardless.
