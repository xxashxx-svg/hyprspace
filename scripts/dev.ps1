# The dev loop: build, run, and rebuild on every save. The app on screen keeps running while a
# build goes; only a build that succeeds closes it and starts the new one, so a typo never leaves
# you without an app. Compile errors print here.
#
#   ./scripts/dev.ps1           run with your real threads, copied once into ~/.hyprspace/dev
#   ./scripts/dev.ps1 -Fresh    start the dev copy over from your current state
#
# The dev copy keeps its own state folder, so it never writes over the installed app's. It starts
# with the same threads and none of them open. Opening one that is open in the other app too puts
# two agents on one conversation.
# Ctrl+C stops the loop and closes the dev app.

param([switch]$Fresh)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$profileDir = [Environment]::GetFolderPath('UserProfile')
$state = Join-Path $profileDir '.hyprspace\dev'
$real = Join-Path $profileDir '.hyprspace\native\state.json'
# the exe that runs is a copy: Windows locks a running exe, and cargo has to write the next one
$runDir = Join-Path $root 'target\dev-run'
$built = Join-Path $root 'target\debug\hyprspace.exe'

if ($Fresh -and (Test-Path $state)) { Remove-Item $state -Recurse -Force }
if (-not (Test-Path (Join-Path $state 'state.json'))) {
    New-Item -ItemType Directory -Force $state | Out-Null
    if (Test-Path $real) {
        # every thread comes along, but no pane on screen: a pane launches its agent at start, and
        # that agent may be live in the installed app on the same conversation
        $copy = Get-Content $real -Raw | ConvertFrom-Json
        # the thread on screen when the app closed opens again at start, so that goes too
        $copy.active = $null
        foreach ($space in $copy.spaces) {
            $space.grid.panes = @()
            $space.grid.focus = $null
            $space.grid.maximized = $null
        }
        $json = $copy | ConvertTo-Json -Depth 100 -Compress
        [IO.File]::WriteAllText((Join-Path $state 'state.json'), $json, (New-Object Text.UTF8Encoding $false))
        Write-Host "Copied your threads into $state" -ForegroundColor DarkGray
    }
}
New-Item -ItemType Directory -Force $runDir | Out-Null

$env:HYPRSPACE_STATE_DIR = $state
$env:HYPRSPACE_DEV = '1'
# a terminal started from an agent can carry these, and the app's own terminals would inherit them
Remove-Item Env:NO_COLOR -ErrorAction SilentlyContinue
Remove-Item Env:CLAUDECODE -ErrorAction SilentlyContinue

$script:app = $null

function Stop-App {
    if (-not $script:app -or $script:app.HasExited) { return }
    # closing the window lets the app end its terminals; a killed app leaves shells and agents behind
    $null = $script:app.CloseMainWindow()
    if (-not $script:app.WaitForExit(8000)) {
        Write-Host 'The app did not close in time, so it was stopped.' -ForegroundColor Yellow
        Stop-Process -Id $script:app.Id -Force -ErrorAction SilentlyContinue
    }
}

function Build-And-Run {
    $t = [Diagnostics.Stopwatch]::StartNew()
    Write-Host ''
    Write-Host 'Building...' -ForegroundColor Cyan
    Push-Location $root
    & cargo build -p hyprspace --locked --color always
    $ok = $LASTEXITCODE -eq 0
    Pop-Location
    if (-not $ok) {
        Write-Host 'Build failed. The app on screen is the last good build.' -ForegroundColor Red
        return
    }
    $build = $t.Elapsed.TotalSeconds
    Stop-App
    $exe = Join-Path $runDir 'hyprspace.exe'
    Copy-Item $built $exe -Force
    $script:app = Start-Process $exe -WorkingDirectory $root -PassThru
    Write-Host ('Running the new build: built in {0:N1}s, swapped in {1:N1}s. Watching for changes.' -f $build, ($t.Elapsed.TotalSeconds - $build)) -ForegroundColor Green
}

# A save under crates/ or apps/ stamps the time. Event actions run in a scope of their own, so the
# stamp lives in a table they are handed.
$flag = [hashtable]::Synchronized(@{ At = $null })
$watchers = @()
$subs = @()
foreach ($dir in 'crates', 'apps') {
    $w = New-Object IO.FileSystemWatcher (Join-Path $root $dir)
    $w.IncludeSubdirectories = $true
    $w.EnableRaisingEvents = $true
    $watchers += $w
    foreach ($kind in 'Changed', 'Created', 'Deleted', 'Renamed') {
        $subs += Register-ObjectEvent $w $kind -MessageData $flag -Action {
            if ($Event.SourceEventArgs.FullPath -match '\.(rs|toml|svg|ttf|png|json)$') {
                $Event.MessageData.At = Get-Date
            }
        }
    }
}

try {
    Build-And-Run
    while ($true) {
        Start-Sleep -Milliseconds 200
        # an editor saves in bursts, and an agent edits several files: wait for a quiet moment
        $at = $flag.At
        if ($at -and ((Get-Date) - $at).TotalMilliseconds -gt 400) {
            $flag.At = $null
            Build-And-Run
        }
    }
}
finally {
    $subs | ForEach-Object { Unregister-Event -SourceIdentifier $_.Name -ErrorAction SilentlyContinue }
    $watchers | ForEach-Object { $_.Dispose() }
    Stop-App
}
