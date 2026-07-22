# Packaging Channels

Installation paths beyond `cargo install` are tracked here for Sprint 7 (L7.3).
Each channel is either implemented or explicitly deferred with an owner.

| Channel | Status | Owner | Notes |
|---|---|---|---|
| Source / `cargo install --path crates/containust-cli` | **Supported** | maintainers | Documented in README and CLI_REFERENCE |
| GitHub Release binaries (`v*` tags) | **Supported** | maintainers | SHA-256 checksums via `.github/workflows/release.yml` |
| Homebrew formula | **Deferred** | maintainers | Blocked on first tagged `0.8.0+` artifact set; track as follow-up issue |
| Debian (`.deb`) | **Deferred** | maintainers | Prefer cargo-dist or nfpm after binary layout stabilizes |
| RPM | **Deferred** | maintainers | Same packaging toolchain as Debian |
| Windows installer (MSI / winget) | **Deferred** | maintainers | Zip artifacts ship today; winget manifest after `1.0.0` |

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

Code signing (cosign / Apple notarization / Authenticode) is deferred until
signing secrets and a release identity are provisioned; checksums remain mandatory.
