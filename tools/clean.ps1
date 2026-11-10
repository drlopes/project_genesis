<#
.SYNOPSIS
  Report and prune build bloat.

.DESCRIPTION
  Default: prune incremental-compilation caches (the fastest-growing part of target/) and
  rust-analyzer's build output, then report sizes. Dependencies stay built, so the next build
  only recompiles our own crates.

  -All: remove target/ entirely (cargo clean). The next build recompiles everything.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File tools/clean.ps1
  powershell -ExecutionPolicy Bypass -File tools/clean.ps1 -All

  (Script execution is disabled system-wide on this machine; -ExecutionPolicy Bypass applies to
  this one process only and changes no system setting.)
#>
param([switch]$All)

$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..')
$target = Join-Path $root 'target'

function Size($path) {
    if (-not (Test-Path $path)) { return '0 MB, 0 files' }
    $files = Get-ChildItem -Recurse -File -Force $path -ErrorAction SilentlyContinue
    '{0:N1} MB, {1:N0} files' -f (($files | Measure-Object Length -Sum).Sum / 1MB), $files.Count
}

Write-Host "Before: target/ = $(Size $target)"

if ($All) {
    Push-Location $root
    cargo clean
    Pop-Location
} elseif (Test-Path $target) {
    $prune = @(Get-ChildItem -Directory -Recurse -Force $target -Filter 'incremental' -ErrorAction SilentlyContinue)
    $prune += @(Join-Path $target 'rust-analyzer' | Where-Object { Test-Path $_ } | Get-Item)
    foreach ($dir in $prune) {
        Remove-Item -Recurse -Force $dir.FullName
        Write-Host "  removed $($dir.FullName.Substring($root.Path.Length + 1))"
    }
}

# Stray per-workspace target dirs left from before the shared target-dir was configured.
$stray = Join-Path $root 'spikes/target'
if (Test-Path $stray) {
    Remove-Item -Recurse -Force $stray
    Write-Host '  removed spikes/target (stray)'
}

Write-Host "After:  target/ = $(Size $target)"

# Stale artefacts (old profiles, a changed linker, earlier dependency versions) are not pruned by
# the default mode and can reach >10 GB with Bevy (seen in E3). Suggest a full clean.
if (-not $All -and (Test-Path $target)) {
    $gb = ((Get-ChildItem -Recurse -File -Force $target -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum) / 1GB
    if ($gb -gt 5) {
        Write-Host ("target/ is still {0:N1} GB: mostly stale artefacts. Run with -All for a full clean (the next build recompiles everything)." -f $gb)
    }
}
