# Release process

`master` is the native Rust application's development and release branch.
`legacy` preserves the original Wallpaper Engine project at its old `master`
commit; a reference copy is also available under `legacy/wallpaper-engine/`.

## Prepare a pre-release

1. Update the package version in `Cargo.toml` and the root package in `Cargo.lock`
   together when making a new version. Add `docs/releases/v<VERSION>.md` with
   features, requirements, limitations and relevant validation.
2. Merge changes into `master`. Wait for the **Windows build** workflow to pass
   formatting, Clippy, native tests, PowerShell 5.1/7 checks and release packaging.
   Test that exact artifact on Windows; CI does not validate desktop composition.
3. Fetch `master` and tag the accepted commit. For the first release:

   ```sh
   git fetch origin master
   git tag -a v0.3.2 origin/master -m "Time Flows Away v0.3.2 pre-release"
   git push origin v0.3.2
   ```

4. Wait for the tag's **Windows build** and **publish** jobs. The workflow rejects
   a tag/version mismatch, a commit outside `master` or missing release notes.
   Publication uses GitHub Actions' own token; no personal token secret is needed.
   Only the publish job has `contents: write`.
5. Check the [release page](https://github.com/trybonk/time-flows-away/releases)
   and download the assets. The ZIP must contain the EXE, README, LICENSE, NOTICE,
   documentation and `build-manifest.json`. Confirm its hash and the EXE hash
   against the separately published checksum and manifest. Confirm the manifest's
   commit matches the tagged source, then smoke-test the downloaded EXE on Windows.

Release downloads are public and do not expire with workflow artifacts. Releases
are marked **Pre-release** and are not promoted to **Latest** automatically. Keep
this policy until Windows acceptance warrants a stable release.

## Verify downloads on Windows

```powershell
Get-FileHash .\time-flows-away-v0.3.2-windows-x64.zip -Algorithm SHA256
# Compare with time-flows-away-v0.3.2-windows-x64.zip.sha256, then extract the ZIP.
$manifest = Get-Content .\build-manifest.json -Raw | ConvertFrom-Json
$actual = (Get-FileHash .\time-flows-away-native.exe -Algorithm SHA256).Hash
if ($actual -ne $manifest.sha256) { throw 'Executable checksum mismatch' }
$manifest | Format-List version, commit, tag, workflowRun, builtAtUtc
```

Checksums verify file integrity, not a publisher signature. The EXE is unsigned;
Windows SmartScreen may ask for confirmation. This release is a portable ZIP,
without an installer or automatic update mechanism. Exit the old version before
replacing its EXE, preserve `%LOCALAPPDATA%\TimeFlowsAway`, and re-enable startup
in Settings if the executable's location changed.

## Failed publication

If checks or packaging fail, no release is published. Read the failed job before
retrying. If upload fails after GitHub has created a release, inspect the existing
release and upload missing verified assets; rerunning `gh release create` will
not replace an existing release. Do not move an already published tag. Fix source
problems in a new commit and version instead.
