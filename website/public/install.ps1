# HyprSpace installer.
#
#   irm https://hyprspace.dev/install.ps1 | iex
#
# Downloads the signed installer from the latest GitHub release and runs it silently. Installs
# per-user, so no admin prompt. Nothing else about your machine changes.

$ErrorActionPreference = 'Stop'

$url = 'https://github.com/xxashxx-svg/hyprspace/releases/latest/download/HyprSpace-windows-x64-setup.exe'
$exe = Join-Path ([System.IO.Path]::GetTempPath()) 'HyprSpace-setup.exe'

Write-Host '==> Downloading HyprSpace...' -ForegroundColor Cyan
# the progress bar makes Invoke-WebRequest an order of magnitude slower on big files
$prev = $ProgressPreference
$ProgressPreference = 'SilentlyContinue'
try {
  Invoke-WebRequest -Uri $url -OutFile $exe -UseBasicParsing
} finally {
  $ProgressPreference = $prev
}

Write-Host '==> Installing...' -ForegroundColor Cyan
$p = Start-Process -FilePath $exe -ArgumentList '/S' -Wait -PassThru
Remove-Item $exe -Force -ErrorAction SilentlyContinue

if ($p.ExitCode -ne 0) { throw "installer exited with $($p.ExitCode)" }

Write-Host '==> Installed. Launch HyprSpace from the Start menu.' -ForegroundColor Green
