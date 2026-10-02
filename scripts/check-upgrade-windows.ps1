<#
.SYNOPSIS
  Let the Tauri app's own updater install the GPUI app, then check what the user ends up with.
  CI only (.github/workflows/upgrade-test.yml): it installs under the real HyprSpace identity, so
  it refuses to run anywhere but a throwaway runner.

.USAGE
  scripts/check-upgrade-windows.ps1 -Old <tauri-setup.exe> -Feed <folder> -Out <folder> [-Port 8765]

  -Old is a Tauri app built from src-tauri with its updater feed at http://127.0.0.1:<Port>.
  -Feed holds latest.json and the signed GPUI installer it names; this script serves it.
  Installs the Tauri app the way a user had it, gives it a project, starts it and waits. Its
  updater has to download the GPUI installer, verify it, run it and quit; the installer has to
  replace it and start the GPUI app, which has to bring the project over. Logs, the state files
  and screenshots land in -Out.
#>
param(
  [Parameter(Mandatory = $true)][string]$Old,
  [Parameter(Mandatory = $true)][string]$Feed,
  [Parameter(Mandatory = $true)][string]$Out,
  [int]$Port = 8765
)

$ErrorActionPreference = "Stop"
if ($env:GITHUB_ACTIONS -ne "true") {
  throw "This installs over the real HyprSpace. It only runs on a CI runner."
}

$Product = "HyprSpace"
$dir = Join-Path $env:LOCALAPPDATA $Product
$tauri = Join-Path $dir "hyprspace-tauri.exe"
$exe = Join-Path $dir "hyprspace.exe"
$uninstKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$Product"
$project = Join-Path $env:RUNNER_TEMP "upgrade-demo"
$v2 = Join-Path $HOME ".hyprspace\v2"
$native = Join-Path $HOME ".hyprspace\native"
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$Out = (Resolve-Path $Out).Path
$Feed = (Resolve-Path $Feed).Path
$manifest = Get-Content (Join-Path $Feed "latest.json") -Raw | ConvertFrom-Json
$version = $manifest.version
$installer = ($manifest.platforms."windows-x86_64".url -split "/")[-1]

function Check($ok, $what) {
  if (-not $ok) { throw "FAILED: $what" }
  Write-Host "ok   $what"
}

function WaitUntil($what, $seconds, [scriptblock]$cond) {
  $deadline = (Get-Date).AddSeconds($seconds)
  while (-not (& $cond)) {
    if ((Get-Date) -gt $deadline) { throw "FAILED: $what (waited ${seconds}s)" }
    Start-Sleep -Milliseconds 500
  }
  Write-Host "ok   $what"
}

function Running($path) {
  @(Get-Process | Where-Object { $_.Path -eq $path })
}

# evidence only: a runner without a desktop shouldn't fail the check
function Shot($name) {
  try {
    Add-Type -AssemblyName System.Windows.Forms, System.Drawing
    $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
    $bmp.Save((Join-Path $Out "$name.png"))
    $g.Dispose(); $bmp.Dispose()
  } catch {
    Write-Host "no screenshot ($name): $_"
  }
}

function WriteJson($path, $value) {
  # serde_json refuses a BOM, which Set-Content -Encoding utf8 writes on Windows PowerShell
  [IO.File]::WriteAllText($path, ($value | ConvertTo-Json -Depth 6 -Compress))
}

$server = $null
try {
  # 1. the Tauri app, installed the way a user had it
  $p = Start-Process -FilePath $Old -ArgumentList "/P" -Wait -PassThru
  Check ($p.ExitCode -eq 0) "the Tauri installer exited 0 (got $($p.ExitCode))"
  Check (Test-Path $tauri) "the Tauri app is installed in $dir"
  $oldVersion = (Get-ItemProperty $uninstKey).DisplayVersion
  Write-Host "     Tauri app version $oldVersion, feed offers $version"

  # 2. a project, in the Tauri app's own store
  New-Item -ItemType Directory -Force -Path $project, $v2 | Out-Null
  WriteJson (Join-Path $v2 "workspaces.json") @{
    workspaces = @(@{
        id = "p1"; name = "upgrade-demo"; cwd = $project; color = "#3fb6e0"; kind = "project"
        sessions = @(); lastOpenedAt = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
      })
    activeId = "p1"
  }
  WriteJson (Join-Path $v2 "settings.json") @{ theme = "iris"; onboarded = $true }
  WriteJson (Join-Path $v2 "lastSeenVersion.json") $oldVersion
  Check (-not (Test-Path (Join-Path $native "state.json"))) "no GPUI state yet"

  # 3. the feed
  $server = Start-Process -FilePath python -PassThru -NoNewWindow `
    -ArgumentList "-u", "-m", "http.server", $Port, "--bind", "127.0.0.1", "--directory", "`"$Feed`"" `
    -RedirectStandardError (Join-Path $Out "feed.log") -RedirectStandardOutput (Join-Path $Out "feed.out.log")
  WaitUntil "the feed answers" 30 {
    try { (Invoke-WebRequest "http://127.0.0.1:$Port/latest.json" -UseBasicParsing).StatusCode -eq 200 } catch { $false }
  }

  # 4. start the Tauri app and let its updater work
  Start-Process -FilePath $tauri | Out-Null
  Start-Sleep -Seconds 3
  Shot "1-tauri-app"
  WaitUntil "the Tauri app quit and its binary is gone" 240 { (-not (Test-Path $tauri)) -and (Running $tauri).Count -eq 0 }
  WaitUntil "the GPUI app is running from $dir" 120 { (Running $exe).Count -gt 0 }
  $leftovers = @(Get-ChildItem $env:TEMP -Directory -Filter "$Product-*-updater-*" -ErrorAction SilentlyContinue)
  Write-Host "     the Tauri updater left $($leftovers.Count) download folder(s) in TEMP"
  Start-Sleep -Seconds 15
  Shot "2-gpui-app"

  # 5. what the user ends up with
  Check ((Running $exe).Count -eq 1) "the GPUI app is still running 15s later"
  Check (Select-String -Path (Join-Path $Out "feed.log") -SimpleMatch "GET /$installer " -Quiet) "the Tauri app downloaded $installer from the feed"
  $exeVersion = (Get-Item $exe).VersionInfo.ProductVersion
  Check ($exeVersion -eq $version) "hyprspace.exe is version $version (got $exeVersion)"
  $entries = @(Get-ChildItem "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall" |
      ForEach-Object { Get-ItemProperty $_.PSPath } |
      Where-Object { $_.DisplayName -eq $Product })
  Check ($entries.Count -eq 1) "Apps has one $Product entry (found $($entries.Count))"
  Check ($entries[0].DisplayVersion -eq $version) "it shows version $version (got $($entries[0].DisplayVersion))"
  Check ($entries[0].MainBinaryName -eq "hyprspace.exe") "it names hyprspace.exe"
  $shell = New-Object -ComObject WScript.Shell
  $start = Join-Path ([Environment]::GetFolderPath("Programs")) "$Product.lnk"
  Check (Test-Path $start) "the Start menu shortcut exists"
  foreach ($lnk in @($start, (Join-Path ([Environment]::GetFolderPath("Desktop")) "$Product.lnk"))) {
    if (Test-Path $lnk) {
      $target = $shell.CreateShortcut($lnk).TargetPath
      Check ($target -eq $exe) "$lnk opens hyprspace.exe (got $target)"
    }
  }
  $stateFile = Join-Path $native "state.json"
  WaitUntil "the GPUI app saved its state" 30 { Test-Path $stateFile }
  $state = Get-Content $stateFile -Raw | ConvertFrom-Json
  $found = @($state.spaces | Where-Object { $_.cwd -and $_.cwd.TrimEnd('\') -eq $project })
  Check ($found.Count -eq 1) "the GPUI app has the project $project (spaces: $(($state.spaces | ForEach-Object { $_.cwd }) -join ', '))"
  Check ($state.appearance.theme -eq "iris") "and the theme (got $($state.appearance.theme))"
  WaitUntil "no updater download is left in TEMP" 60 {
    @(Get-ChildItem $env:TEMP -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -like "hyprspace-update-*" -or $_.Name -like "$Product-*-updater-*" }).Count -eq 0
  }
  Write-Host "all good: the Tauri app $oldVersion updated itself into the GPUI app $version"
} finally {
  Shot "3-end"
  Copy-Item -Recurse -Force $v2 (Join-Path $Out "v2") -ErrorAction SilentlyContinue
  Copy-Item -Force (Join-Path $native "state.json") $Out -ErrorAction SilentlyContinue
  Get-ItemProperty $uninstKey -ErrorAction SilentlyContinue | Out-File (Join-Path $Out "apps-entry.txt")
  Get-ChildItem $dir -ErrorAction SilentlyContinue | Out-File (Join-Path $Out "install-folder.txt")
  Get-ChildItem $env:TEMP -Directory -ErrorAction SilentlyContinue | Out-File (Join-Path $Out "temp.txt")
  foreach ($p in (Running $exe) + (Running $tauri)) {
    [void]$p.CloseMainWindow()
    if (-not $p.WaitForExit(5000)) { $p.Kill() }
  }
  if ($server) { Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue }
}
