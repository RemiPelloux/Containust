# winget manifest (P10.16 / P11.8)

Manifest for publishing the Windows zip to
[winget-pkgs](https://github.com/microsoft/winget-pkgs).

## Per release (automated)

The `packaging-bump` job in `.github/workflows/release.yml` refreshes
`PackageVersion`, `InstallerUrl`, and `InstallerSha256` from the release
`SHA256SUMS` via [`scripts/bump_packaging.sh`](../../scripts/bump_packaging.sh)
and opens a PR on Containust.

After that PR merges (or from the bump branch), submit to winget-pkgs:

```powershell
wingetcreate validate .\packaging\winget\Containust.ctst.yaml
wingetcreate submit .\packaging\winget\Containust.ctst.yaml
```

Or open a PR under `manifests/c/Containust/ctst/<version>/` in
`microsoft/winget-pkgs` with the updated singleton YAML.

## Manual refresh

```bash
./scripts/bump_packaging.sh 1.2.0 ./SHA256SUMS
```
