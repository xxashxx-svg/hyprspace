<#
.SYNOPSIS
  Cut an Android app release on its own: bump mobile/VERSION, commit and tag android-v<new>, open
  a draft GitHub release with the notes, then run android.yml. CI tests, builds and signs the APK,
  attaches it and publishes the release without marking it the repo's latest, which stays the
  desktop's for its updater.

.USAGE
  .\mobile\deploy.ps1 patch "One bullet per line`nAnother bullet"
  .\mobile\deploy.ps1 current "Release mobile/VERSION as it is"

.REQUIRES
  - a clean working tree on main
  - gh logged in (gh auth status)
#>
param(
  [ValidateSet("patch", "minor", "major", "current")]
  [string]$Bump = "patch",
  [Parameter(Mandatory = $true)]
  [string]$Notes
)

$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot -Parent)
$Repo = "xxashxx-svg/hyprspace"

function Run($cmd) {
  Write-Host ">> $cmd" -ForegroundColor DarkGray
  Invoke-Expression $cmd
  if ($LASTEXITCODE -ne 0) { throw "failed: $cmd" }
}

if ((git status --porcelain --untracked-files=no) -ne $null) { throw "The working tree has uncommitted changes. Commit or stash first." }
$branch = (git rev-parse --abbrev-ref HEAD).Trim()
if ($branch -ne "main") { throw "Release from main, not $branch." }
gh auth status 2>$null | Out-Null
if ($LASTEXITCODE -ne 0) { throw "gh is not logged in. Run: gh auth login" }

$bullets = @($Notes -split "(?:`r?`n)|\s\|\s" | ForEach-Object { $_.Trim().TrimStart("-", "*", " ") } | Where-Object { $_ -ne "" })
if ($bullets.Count -eq 0) { throw "No release notes given." }

$cur = (Get-Content mobile/VERSION -Raw).Trim()
if ($cur -notmatch '^[0-9]+\.[0-9]+\.[0-9]+$') { throw "mobile/VERSION isn't a version: $cur" }
$parts = $cur.Split(".") | ForEach-Object { [int]$_ }
switch ($Bump) {
  "major" { $parts = @(($parts[0] + 1), 0, 0) }
  "minor" { $parts = @($parts[0], ($parts[1] + 1), 0) }
  "patch" { $parts = @($parts[0], $parts[1], ($parts[2] + 1)) }
}
$new = ($parts -join ".")
$tag = "android-v$new"
if (git tag -l $tag) { throw "$tag already exists." }
Write-Host "Releasing the Android app $cur -> $new" -ForegroundColor Cyan

if ($Bump -ne "current") {
  [IO.File]::WriteAllText((Join-Path (Get-Location) "mobile/VERSION"), "$new`n", (New-Object Text.UTF8Encoding($false)))
  Run "git add mobile/VERSION"
  Run "git commit -q -m `"android: $tag`""
}
Run "git tag $tag"
Run "git push origin main"
Run "git push origin $tag"

$body = "## What's changed`n`n" + (($bullets | ForEach-Object { "- $_" }) -join "`n") + "`n"
$notesFile = Join-Path $env:TEMP "hyprspace-android-notes.md"
[IO.File]::WriteAllText($notesFile, $body, (New-Object Text.UTF8Encoding($false)))

Run "gh release create $tag --repo $Repo --draft --latest=false --title `"HyprSpace for Android $new`" --notes-file `"$notesFile`""
Run "gh workflow run android.yml --repo $Repo -f tag=$tag"

Write-Host ""
Write-Host "Draft opened: https://github.com/$Repo/releases/tag/$tag" -ForegroundColor Green
Write-Host "CI builds and signs the APK, then publishes it: gh run watch --repo $Repo" -ForegroundColor Green
