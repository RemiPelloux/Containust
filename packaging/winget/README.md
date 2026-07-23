# winget manifest (P10.16)

Manifest template for publishing the Windows zip to
[winget-pkgs](https://github.com/microsoft/winget-pkgs). Submit with:

```powershell
wingetcreate submit .\packaging\winget\Containust.ctst.yaml
```

Update `PackageVersion`, `InstallerUrl`, and `InstallerSha256` (from the
release `SHA256SUMS`) for each release before submitting.
