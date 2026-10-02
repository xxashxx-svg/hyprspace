# ADR 0010: Installers that take over the Tauri app's identity

- Status: Accepted
- Date: 2026-10-02

## Context

Every current user updates through the Tauri app's updater (tauri-plugin-updater 2.10). On
Windows it downloads the NSIS installer `latest.json` names, checks its minisign signature, runs
it as `setup.exe /P /R /UPDATE /ARGS` through `ShellExecute` and exits. On macOS it unpacks the
`.app.tar.gz` over the running bundle, dropping the archive's top folder, and relaunches the
binary that the new bundle's `CFBundleExecutable` names (tauri 2.11's `restart_macos_app`). The
first GPUI release has to work through exactly that, or users stay on the last Tauri version.

## Decision

**Windows: our own NSIS script with the Tauri installer's identity**
(`apps/hyprspace/package/windows/installer.nsi`, built by `scripts/package-windows.ps1`). It
writes the same per-user keys the Tauri installer wrote (`HKCU\...\Uninstall\HyprSpace`,
publisher `hyprspace`, `HKCU\Software\hyprspace\HyprSpace` for the folder) into the same folder
(`%LOCALAPPDATA%\HyprSpace`, or wherever that key points). So Apps keeps one HyprSpace entry and
its uninstaller is ours. The binary is `hyprspace.exe`, not `hyprspace-tauri.exe`: the installer
reads the old name from `MainBinaryName`, deletes that file, and points the Start menu, desktop
and pinned taskbar shortcuts at the new one, the same migration Tauri's own template does when a
binary is renamed. Shortcuts carry the AppUserModelID `com.hyprspace.app`, which the app also
sets on its process, so its window groups with a pinned HyprSpace.

It takes the flags Tauri's updater passes and nothing changes for them: `/P` passive, `/R`
relaunch with `/ARGS`, `/UPDATE` (wait for the app that just quit, keep the user's shortcut
choices), plus `/S`, `/NS` and `/D=`. The GPUI app's own updater passes the same four, so there is
one way an updater runs the installer.

**Closing a running app is scoped to its path.** In update mode the installer waits up to 10s for
the files to unlock. Past that, or on a manual install, it finds processes whose image is the file
in the install folder, sends their window `WM_CLOSE` (the app then ends its sessions and kills its
PTYs the normal way), and kills only what is still there 5s later. A manual install asks first.
Tauri's template matched by process name, which would close every HyprSpace on the machine; ours
leaves another install alone. The installer is 32-bit, and a 32-bit PowerShell can't read a 64-bit
process's path, so it runs the 64-bit one through `Sysnative`.

**Test identity.** `package-windows.ps1 -Test` builds "HyprSpace Test" with its own name,
publisher, keys, folder and AppUserModelID, so the installer can be installed, updated and
uninstalled on a machine that has the real app. `scripts/check-windows-install.ps1` runs the
install, upgrade-over-an-old-install and uninstall checks; release CI runs it with the real
identity over the real v0.21.1 Tauri installer on a throwaway runner.

**macOS: a hand-assembled bundle** (`scripts/package-macos.sh`, after zeron's): `HyprSpace.app`
with `CFBundleIdentifier` `com.hyprspace.app` and `CFBundleExecutable` `hyprspace`, the
`.app.tar.gz` with one top-level `HyprSpace.app` (no AppleDouble files), and a dmg. It signs with
the Developer ID and notarizes when the Apple secrets exist, otherwise signs ad hoc, which Apple
silicon needs to launch it at all. `LSMinimumSystemVersion` is 11.0: only `darwin-aarch64` ships,
and every Apple silicon Mac came with 11 or later.

**Names stay the Tauri releases' names** (`HyprSpace_<version>_x64-setup.exe`,
`HyprSpace.app.tar.gz`, `HyprSpace_<version>_aarch64.dmg` and the stable website copies), so
`scripts/ci-build-latest.mjs`, `latest.json` and the website need no change.

## Rejected

- **tauri-bundler for the GPUI app.** It wants a Tauri config and app, and pulls the npm setup we
  are deleting. Its template is where ours started (`utils.nsh` keeps its shortcut macros).
- **Inno Setup, as zeron uses.** The Tauri app's uninstaller and registry layout are NSIS's; an
  NSIS installer reads and replaces them with no translation, and NSIS is already on the machine
  that built every Tauri release.
- **Calling the binary `hyprspace-tauri.exe`.** It would skip the shortcut migration, and carry a
  dead name forever.
- **Deleting the Tauri app's WebView2 data** (`%LOCALAPPDATA%\com.hyprspace.app`). It is the old
  app's cache, not ours to judge; the uninstaller never touches user data (`~/.hyprspace`) either.

## Unproven here

The macOS bundle, Developer ID signing and notarization only run in CI. The Tauri app's own
updater running this installer is the next phase's test; this phase checked the same command line
by hand and through the GPUI app's updater.
