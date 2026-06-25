# deploy.ps1 — ship a HyprSpace update: bump version, build + sign the installer,
# and publish a GitHub release carrying the auto-update manifest.
#
#   .\deploy.ps1 patch "Fixed the thing"   # 0.1.0 -> 0.1.1   (bug fix / polish)
#   .\deploy.ps1 minor "New feature"       # 0.1.0 -> 0.2.0   (new user-facing feature)
#   .\deploy.ps1 major "Big change"        # 0.1.0 -> 1.0.0   (milestone / 1.0 / breaking)
#   .\deploy.ps1 none  "First release"     # publish the current version as-is
#
# Pick the bump by docs/VERSIONING.md. Full release steps (and how to do it without Claude):
# docs/DEPLOY.md. Never hand-edit version numbers — this script keeps all three files in sync.
param(
  [ValidateSet('none', 'patch', 'minor', 'major')][string]$bump = 'patch',
  [string]$notes = "",
  [switch]$Mac   # macOS CI is paused for now; pass -Mac to also build + publish the darwin artifacts
)
$ErrorActionPreference = "Stop"
$root = $PSScriptRoot
$relRepo = "xxashxx-svg/hyprspace-releases"
$sigDir = Join-Path $env:USERPROFILE ".hyprspace-signing"

function Set-Version($file, $pattern, $replacement) {
  $t = [System.IO.File]::ReadAllText($file)
  $t = [regex]::Replace($t, $pattern, $replacement)
  [System.IO.File]::WriteAllText($file, $t)
}

$confPath = Join-Path $root "src-tauri\tauri.conf.json"
$pkgPath = Join-Path $root "package.json"
$cargoPath = Join-Path $root "src-tauri\Cargo.toml"
$cur = [version]((Get-Content $confPath -Raw | ConvertFrom-Json).version)

if ($bump -eq 'none') { $new = "$($cur.Major).$($cur.Minor).$($cur.Build)" }
elseif ($bump -eq 'major') { $new = "$($cur.Major + 1).0.0" }
elseif ($bump -eq 'minor') { $new = "$($cur.Major).$($cur.Minor + 1).0" }
else { $new = "$($cur.Major).$($cur.Minor).$($cur.Build + 1)" }
Write-Host "==> Version: $cur -> $new" -ForegroundColor Cyan

if ($bump -ne 'none') {
  Set-Version $confPath '("version":\s*")[^"]+(")' ('${1}' + $new + '${2}')
  Set-Version $pkgPath '("version":\s*")[^"]+(")' ('${1}' + $new + '${2}')
  Set-Version $cargoPath '(?m)^version\s*=\s*"[^"]+"' ('version = "' + $new + '"')
}

if (-not (Test-Path "$sigDir\hyprspace.key")) { throw "signing key not found at $sigDir\hyprspace.key" }
$env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content "$sigDir\hyprspace.key" -Raw).Trim()
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = (Get-Content "$sigDir\password.txt" -Raw).Trim()

Write-Host "==> Building signed installer (a few minutes)..." -ForegroundColor Cyan
Push-Location $root
npm run tauri build
$code = $LASTEXITCODE
Pop-Location
if ($code -ne 0) { throw "build failed (exit $code)" }

$nsisDir = Join-Path $root "src-tauri\target\release\bundle\nsis"
$setup = Get-ChildItem $nsisDir -Filter "*-setup.exe" | Sort-Object LastWriteTime -Descending | Select-Object -First 1
$sig = Get-ChildItem $nsisDir -Filter "*-setup.exe.sig" | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $setup -or -not $sig) { throw "installer/signature not found in $nsisDir" }

# stable-named copy so the website's "Download" link (releases/latest/download/HyprSpace-windows-x64-setup.exe)
# resolves to the newest installer on every release without hardcoding the version
$dlAlias = Join-Path $nsisDir "HyprSpace-windows-x64-setup.exe"
Copy-Item $setup.FullName $dlAlias -Force

$tag = "v$new"
$url = "https://github.com/$relRepo/releases/download/$tag/$($setup.Name)"
if ([string]::IsNullOrWhiteSpace($notes)) { $notes = "HyprSpace $new" }
$manifest = [ordered]@{
  version   = $new
  notes     = $notes
  pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
  platforms = [ordered]@{
    "windows-x86_64" = [ordered]@{ signature = (Get-Content $sig.FullName -Raw).Trim(); url = $url }
  }
}
$artDir = Join-Path $root "release-artifacts"
New-Item -ItemType Directory -Force -Path $artDir | Out-Null
$latest = Join-Path $artDir "latest.json"
$manifest | ConvertTo-Json -Depth 6 | Set-Content $latest -Encoding UTF8

if ($bump -ne 'none') {
  try {
    git -C $root add -A
    git -C $root commit -m "release $tag" | Out-Null
    git -C $root push | Out-Null
  }
  catch { Write-Warning "git commit/push skipped: $_" }
}

Write-Host "==> Publishing $tag to $relRepo ..." -ForegroundColor Cyan
gh release create $tag --repo $relRepo --title "HyprSpace $new" --notes $notes $setup.FullName $dlAlias $latest
if ($LASTEXITCODE -ne 0) { throw "gh release create failed" }

# kick off the macOS CI build (merges the darwin entry into this release's manifest) — paused for now
if ($Mac) {
  Write-Host "==> Triggering macOS CI build for $tag ..." -ForegroundColor Cyan
  gh workflow run release.yml --repo "xxashxx-svg/hyprspace-2" --ref main -f tag=$tag
  if ($LASTEXITCODE -ne 0) { Write-Warning "macOS CI not triggered — run manually: gh workflow run release.yml -f tag=$tag" }
}
else {
  Write-Host "==> Skipping macOS CI (paused). Pass -Mac to build + publish darwin artifacts." -ForegroundColor DarkGray
}

$macMsg = if ($Mac) { " macOS build is queued — it'll add the darwin entry to the manifest shortly." } else { " (Windows only; macOS paused.)" }
Write-Host "==> Done. $tag is live (Windows).$macMsg" -ForegroundColor Green
Write-Host "    Installer: $($setup.Name)"
