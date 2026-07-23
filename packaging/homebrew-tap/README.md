# Homebrew tap bootstrap (P11.7)

Dedicated tap for `ctst`:

```text
brew tap RemiPelloux/containust
brew install ctst
```

## One-time setup

1. Create a public GitHub repository named **`homebrew-containust`** under
   `RemiPelloux` (Homebrew requires the `homebrew-` prefix).
2. Copy [`Formula/ctst.rb`](../../Formula/ctst.rb) into
   `Formula/ctst.rb` in that repository and commit.
3. In **Containust → Settings → Secrets**, add:
   - `HOMEBREW_TAP_TOKEN` — classic PAT (or fine-grained) with
     `contents:write` on `RemiPelloux/homebrew-containust`.

## Automated updates

On every `v*` release, `.github/workflows/release.yml`:

1. Builds artifacts and publishes `SHA256SUMS`.
2. Runs [`scripts/bump_packaging.sh`](../../scripts/bump_packaging.sh) to
   refresh in-tree `Formula/ctst.rb` + winget SHA (opens a PR on Containust).
3. If `HOMEBREW_TAP_TOKEN` is set, pushes the updated formula to
   `homebrew-containust`.

Until the tap repo and secret exist, installs still work via the in-tree
formula:

```bash
brew install --formula ./Formula/ctst.rb
```
