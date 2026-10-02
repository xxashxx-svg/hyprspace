<#
.SYNOPSIS
  Install, check and uninstall a HyprSpace installer, optionally over an older install first.
  Release CI runs it on a throwaway runner (release.yml, dry run); locally, only ever run it with
  a -Test installer (scripts/package-windows.ps1 -Test), whose identity no real install shares.

.USAGE
  scripts/check-windows-install.ps1 -Setup <new-setup.exe> -Dir <empty folder> [-Old <old-setup.exe>]
      [-Product "HyprSpace"] [-Publisher hyprspace] [-OldBinary hyprspace-tauri.exe]

  With -Old, the old installer goes into -Dir first (passive, the way a user had it), then the
  new one runs the way the Tauri app's updater runs it (/P /UPDATE, no folder given), so it has
  to find the old install on its own. Then: one Apps entry with the new version, hyprspace.exe in
  place of the old binary, every shortcut opening it. Then the uninstaller has to leave nothing.
#>
param(
  [Parameter(Mandatory = $true)][string]$Setup,
  [Parameter(Mandatory = $true)][string]$Dir,
  [string]$Old = "",
  [string]$Product = "HyprSpace",
  [string]$Publisher = "hyprspace",
  [string]$OldBinary = "hyprspace-tauri.exe"
)

$ErrorActionPreference = "Stop"
$uninstKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$Product"
$shell = New-Object -ComObject WScript.Shell
$links = @(
  (Join-Path ([Environment]::GetFolderPath("Programs")) "$Product.lnk"),
  (Join-Path ([Environment]::GetFolderPath("Desktop")) "$Product.lnk")
)

function Check($ok, $what) {
  if (-not $ok) { throw "FAILED: $what" }
  Write-Host "ok   $what"
}

function Run($exe, $argList) {
  Write-Host ">> $exe $argList"
  $p = Start-Process -FilePath $exe -ArgumentList $argList -Wait -PassThru
  Check ($p.ExitCode -eq 0) "$(Split-Path $exe -Leaf) exited 0 (got $($p.ExitCode))"
}

function Entries {
  @(Get-ChildItem "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall" |
      ForEach-Object { Get-ItemProperty $_.PSPath } |
      Where-Object { $_.DisplayName -eq $Product })
}

$Dir = [IO.Path]::GetFullPath($Dir)
$version = (Get-Item $Setup).VersionInfo.ProductVersion

if ($Old) {
  Run $Old "/P /D=$Dir"
  Check (Test-Path (Join-Path $Dir $OldBinary)) "the old app is installed"
  Run $Setup "/P /UPDATE"
  Check (-not (Test-Path (Join-Path $Dir $OldBinary))) "the old binary is gone"
} else {
  Run $Setup "/S /D=$Dir"
}

$exe = Join-Path $Dir "hyprspace.exe"
Check (Test-Path $exe) "hyprspace.exe is in $Dir"
Check (Test-Path (Join-Path $Dir "uninstall.exe")) "the uninstaller is beside it"
$entries = @(Entries)
Check ($entries.Count -eq 1) "Apps has one $Product entry (found $($entries.Count))"
$e = $entries[0]
Check ($e.DisplayVersion -eq $version) "it shows version $version (got $($e.DisplayVersion))"
Check ($e.MainBinaryName -eq "hyprspace.exe") "it names hyprspace.exe"
Check ($e.InstallLocation.Trim('"') -eq $Dir) "it points at $Dir"
foreach ($lnk in $links) {
  if (Test-Path $lnk) {
    $target = $shell.CreateShortcut($lnk).TargetPath
    Check ($target -eq $exe) "$lnk opens hyprspace.exe (got $target)"
  }
}
Check (Test-Path $links[0]) "the Start menu shortcut exists"

# The uninstaller copies itself to %TEMP% and runs from there, so wait for its last step: the
# Apps entry going away.
& (Join-Path $Dir "uninstall.exe") /S
$deadline = (Get-Date).AddSeconds(60)
while (Test-Path $uninstKey) {
  if ((Get-Date) -gt $deadline) { break }
  Start-Sleep -Milliseconds 500
}
Check (-not (Test-Path $Dir)) "the uninstaller removed $Dir"
Check (-not (Test-Path $uninstKey)) "and the Apps entry"
Check (-not (Test-Path "HKCU:\Software\$Publisher\$Product")) "and the install folder's key"
foreach ($lnk in $links) { Check (-not (Test-Path $lnk)) "and $lnk" }
Write-Host "all good"
