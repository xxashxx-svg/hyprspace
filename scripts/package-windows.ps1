<#
.SYNOPSIS
  Build the Windows release of the GPUI app: hyprspace.exe, then its NSIS installer.

.USAGE
  scripts/package-windows.ps1                 # cargo build --release, then the installer
  scripts/package-windows.ps1 -Binary <exe>   # package an exe you already built
  scripts/package-windows.ps1 -Test           # a "HyprSpace Test" installer (see below)

  Writes target/package/HyprSpace_<version>_x64-setup.exe, the name every Tauri release used, so
  latest.json, the website's download link and the updaters keep finding it. The version is the
  workspace's (Cargo.toml), the one deploy.ps1 bumps.

  -Test gives the installer its own name, registry keys, folder and shortcut ("HyprSpace Test",
  publisher hyprspace-test), so it can be installed, updated and uninstalled on a machine that
  has the real HyprSpace without touching it.

.REQUIRES
  NSIS 3 (makensis): on PATH, in Program Files, or the copy the Tauri CLI keeps in
  %LOCALAPPDATA%\tauri\NSIS. CI installs it with choco.
#>
param(
  [string]$Out = "target/package",
  [string]$Binary = "",
  [switch]$Test
)

$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
  if (-not $Binary) {
    cargo build --release --locked -p hyprspace
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
    $Binary = "target/release/hyprspace.exe"
  }
  $Binary = (Resolve-Path $Binary).Path

  $meta = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
  $version = ($meta.packages | Where-Object { $_.name -eq "hyprspace" }).version
  if (-not $version) { throw "couldn't read the app's version from cargo metadata" }

  $makensis = @(
    (Get-Command makensis.exe -ErrorAction SilentlyContinue).Source,
    "${env:ProgramFiles(x86)}\NSIS\makensis.exe",
    "$env:ProgramFiles\NSIS\makensis.exe",
    "$env:LOCALAPPDATA\tauri\NSIS\makensis.exe"
  ) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
  if (-not $makensis) { throw "NSIS isn't installed: choco install nsis, or winget install NSIS.NSIS" }

  New-Item -ItemType Directory -Force -Path $Out | Out-Null
  $name = if ($Test) { "HyprSpaceTest" } else { "HyprSpace" }
  $setup = Join-Path (Resolve-Path $Out).Path "${name}_${version}_x64-setup.exe"
  $icon = (Resolve-Path "apps/hyprspace/assets/hyprspace.ico").Path

  $defines = @("/DVERSION=$version", "/DBINARY=$Binary", "/DICON=$icon", "/DOUTFILE=$setup")
  if ($Test) {
    $defines += @("/DPRODUCTNAME=HyprSpace Test", "/DMANUFACTURER=hyprspace-test", "/DBUNDLEID=com.hyprspace.test")
  }
  & $makensis /V2 @defines "apps/hyprspace/package/windows/installer.nsi"
  if ($LASTEXITCODE -ne 0) { throw "makensis failed" }
  Write-Host "packaged $setup"
} finally {
  Pop-Location
}
