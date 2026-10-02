<#
.SYNOPSIS
  Cut a HyprSpace release: bump the version, write the changelog entry, commit and tag, open a
  draft GitHub release with the notes, then run the release workflow. CI builds the GPUI app for
  Windows and macOS, signs what the updaters install with the updater key it holds as a secret,
  fills in latest.json, and publishes the draft once every platform is in. No machine needs the
  signing key.

.USAGE
  .\deploy.ps1 patch "One bullet per line`nAnother bullet"
  .\deploy.ps1 minor "Bullet one | Bullet two"

  Bullets are split on newlines or " | ". They become docs/CHANGELOG.md, the release notes, and
  the updater manifest's notes (the in-app "What's new" reads the bundled changelog).

.REQUIRES
  - a clean working tree on main
  - gh logged in (gh auth status)
#>
param(
  [ValidateSet("patch", "minor", "major")]
  [string]$Bump = "patch",
  [Parameter(Mandatory = $true)]
  [string]$Notes
)

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
$Repo = "xxashxx-svg/hyprspace"

function Run($cmd) {
  Write-Host ">> $cmd" -ForegroundColor DarkGray
  Invoke-Expression $cmd
  if ($LASTEXITCODE -ne 0) { throw "failed: $cmd" }
}

# ---- preflight
if ((git status --porcelain --untracked-files=no) -ne $null) { throw "The working tree has uncommitted changes. Commit or stash first." }
$branch = (git rev-parse --abbrev-ref HEAD).Trim()
if ($branch -ne "main") { throw "Release from main, not $branch." }
gh auth status 2>$null | Out-Null
if ($LASTEXITCODE -ne 0) { throw "gh is not logged in. Run: gh auth login" }

# ---- bullets
$bullets = @($Notes -split "(?:`r?`n)|\s\|\s" | ForEach-Object { $_.Trim().TrimStart("-", "*", " ") } | Where-Object { $_ -ne "" -and $_ -notmatch "^\s*$" })
if ($bullets.Count -eq 0) { throw "No release notes given." }

# ---- version: the Cargo workspace's is the app's (docs/VERSIONING.md)
$cargo = Get-Content Cargo.toml -Raw -Encoding UTF8
$cur = [regex]::Match($cargo, '(?m)^version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"').Groups[1].Value
if (-not $cur) { throw "No version in Cargo.toml's [workspace.package]." }
$parts = $cur.Split(".") | ForEach-Object { [int]$_ }
switch ($Bump) {
  "major" { $parts = @(($parts[0] + 1), 0, 0) }
  "minor" { $parts = @($parts[0], ($parts[1] + 1), 0) }
  "patch" { $parts = @($parts[0], $parts[1], ($parts[2] + 1)) }
}
$new = ($parts -join ".")
$tag = "v$new"
Write-Host "Releasing $cur -> $new" -ForegroundColor Cyan

function Set-Text($path, $text) {
  $full = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($path) # works for new files too
  [IO.File]::WriteAllText($full, $text, (New-Object Text.UTF8Encoding($false)))
}
# one bump, formatting untouched: the workspace's version, which every crate inherits
Set-Text Cargo.toml ([regex]::new('(?m)^version\s*=\s*"' + [regex]::Escape($cur) + '"').Replace($cargo, "version = `"$new`"", 1))
# the lock file records each workspace crate's version; keep it in step so CI's --locked build passes
$lock = Get-Content Cargo.lock -Raw -Encoding UTF8
Set-Text Cargo.lock ([regex]::new('(name = "hyprspace(?:-[a-z]+)?"\r?\nversion = ")' + [regex]::Escape($cur) + '(")').Replace($lock, '${1}' + $new + '${2}'))
$files = @("Cargo.toml", "Cargo.lock")
# the Tauri app's three version files move along until it is deleted (docs/REWRITE.md)
if (Test-Path src-tauri/tauri.conf.json) {
  $pkg = Get-Content package.json -Raw -Encoding UTF8
  Set-Text package.json ([regex]::new('"version":\s*"' + [regex]::Escape($cur) + '"').Replace($pkg, "`"version`": `"$new`"", 1))
  $tc = Get-Content src-tauri/tauri.conf.json -Raw -Encoding UTF8
  Set-Text src-tauri/tauri.conf.json ([regex]::new('"version":\s*"' + [regex]::Escape($cur) + '"').Replace($tc, "`"version`": `"$new`"", 1))
  $tcargo = Get-Content src-tauri/Cargo.toml -Raw -Encoding UTF8
  Set-Text src-tauri/Cargo.toml ([regex]::new('(?m)^version\s*=\s*"' + [regex]::Escape($cur) + '"').Replace($tcargo, "version = `"$new`"", 1))
  $tlock = Get-Content src-tauri/Cargo.lock -Raw -Encoding UTF8
  Set-Text src-tauri/Cargo.lock ([regex]::new('(name = "hyprspace-tauri"\r?\nversion = ")' + [regex]::Escape($cur) + '(")').Replace($tlock, '${1}' + $new + '${2}', 1))
  $files += @("package.json", "src-tauri/tauri.conf.json", "src-tauri/Cargo.toml", "src-tauri/Cargo.lock")
}

# ---- changelog: a new section above the previous one, before the build so the bundle carries it
$date = Get-Date -Format "yyyy-MM-dd"
$dash = [string][char]0x2014
$entry = "## $new $dash $date`n`n" + (($bullets | ForEach-Object { "- $_" }) -join "`n") + "`n`n"
$log = Get-Content docs/CHANGELOG.md -Raw -Encoding UTF8
$at = $log.IndexOf("`n## ")
if ($at -lt 0) { $log = $log.TrimEnd() + "`n`n" + $entry } else { $log = $log.Substring(0, $at + 1) + $entry + $log.Substring($at + 1) }
Set-Text docs/CHANGELOG.md $log

# ---- commit, tag, push
Run "git add $($files -join ' ') docs/CHANGELOG.md"
Run "git commit -q -m `"release: $tag`""
Run "git tag $tag"
Run "git push origin main"
Run "git push origin $tag"

# ---- the release body, laid out like GitHub's own generated notes: what changed, who made it
# (GitHub turns the @mentions into a row of avatars), and the full diff. The contributors are the
# accounts behind the commits since the last tag, so it only ever names people who did the work.
# CI copies this body into the updater manifest too.
$prev = "v$cur"
$logins = @(gh api "repos/$Repo/compare/$prev...$tag" --jq ".commits[].author.login" 2>$null | Where-Object { $_ } | Sort-Object -Unique)
$body = "## What's changed`n`n" + (($bullets | ForEach-Object { "- $_" }) -join "`n") + "`n"
if ($logins.Count -gt 0) { $body += "`n## Contributors`n`n" + (($logins | ForEach-Object { "@$_" }) -join " ") + "`n" }
$body += "`n**Full changelog**: https://github.com/$Repo/compare/$prev...$tag`n"
$notesFile = Join-Path $env:TEMP "hyprspace-release-notes.md"
Set-Text $notesFile $body

# ---- a draft release with the notes; CI attaches the builds and publishes it when all are in
Run "gh release create $tag --repo $Repo --draft --title `"HyprSpace $new`" --notes-file `"$notesFile`""
Run "gh workflow run release.yml --repo $Repo -f tag=$tag"

Write-Host ""
Write-Host "Draft opened: https://github.com/$Repo/releases/tag/$tag" -ForegroundColor Green
Write-Host "CI is building Windows and macOS and will publish it: gh run watch --repo $Repo" -ForegroundColor Green
