# Experimental eframe workaround removal. Never changes the normal checkout.
param([string]$OutputRoot = (Join-Path ([IO.Path]::GetTempPath()) ('tfa-gl-prototype-' + [guid]::NewGuid())), [string]$Target = '', [switch]$CheckOnly)
$ErrorActionPreference = 'Stop'
$repository = Split-Path $PSScriptRoot -Parent
Push-Location $repository
try {
    $metadata = cargo metadata --locked --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed' }
    $eframe = @($metadata.packages | Where-Object { $_.name -eq 'eframe' -and $_.version -eq '0.33.3' })
    if ($eframe.Count -ne 1) { throw 'Prototype reviewed only for eframe 0.33.3; review upstream code first.' }
    if (Test-Path $OutputRoot) { throw 'OutputRoot already exists; use a new directory.' }
    New-Item -ItemType Directory $OutputRoot | Out-Null
    $copy = Join-Path $OutputRoot 'project'
    New-Item -ItemType Directory $copy | Out-Null
    Copy-Item src, Cargo.toml, Cargo.lock, build.rs, rust-toolchain.toml -Destination $copy -Recurse
    $vendor = Join-Path $OutputRoot 'eframe'
    Copy-Item (Split-Path $eframe[0].manifest_path -Parent) -Destination $vendor -Recurse
    $integration = Join-Path $vendor 'src/native/glow_integration.rs'
    $source = [IO.File]::ReadAllText($integration)
    $guard = 'if !cfg!(target_os = "windows") {'
    if ([regex]::Matches($source, [regex]::Escape($guard)).Count -ne 1) { throw 'Expected context guard changed; refusing patch.' }
    $source = $source.Replace($guard, 'if true { // EXPERIMENT: enable is_current fast path on Windows')
    [IO.File]::WriteAllText($integration, $source, [Text.UTF8Encoding]::new($false))
    $patch = 'patch.crates-io.eframe.path=' + (($vendor.Replace('\', '/')) | ConvertTo-Json -Compress)
    $buildArgs = @($(if ($CheckOnly) { 'check' } else { 'build' }), '--manifest-path', (Join-Path $copy 'Cargo.toml'), '--config', $patch)
    if ($CheckOnly) { $buildArgs += '--all-targets' } else { $buildArgs += '--release' }
    if ($Target) { $buildArgs += @('--target', $Target) }
    $oldCommit = $env:TFA_BUILD_COMMIT
    try {
        $env:TFA_BUILD_COMMIT = (git rev-parse HEAD) + "-experimental"
        cargo @buildArgs
        $buildExit = $LASTEXITCODE
    } finally { $env:TFA_BUILD_COMMIT = $oldCommit }
    if ($buildExit -ne 0) { throw 'Experimental build failed' }
    [ordered]@{ experimental = $true; eframe = '0.33.3'; change = 'Windows is_current fast path'; sourceCommit = (git rev-parse HEAD); sourceDirty = [bool](git status --porcelain); target = $Target; checkedOnly = [bool]$CheckOnly } | ConvertTo-Json | Set-Content (Join-Path $OutputRoot 'prototype-manifest.json') -Encoding UTF8
    Write-Host "Prototype: $OutputRoot. Test CPU AND rendering/recovery regressions before considering this workaround change."
} finally { Pop-Location }
