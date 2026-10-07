# Updates and installers

## Why it all looks like the Tauri app

HyprSpace was a Tauri app until v0.21.1. Users on that version update through the Tauri updater,
which reads `releases/latest/download/latest.json`, downloads the installer it names, checks its
minisign signature against the key the Tauri app shipped with, and runs it. Every release has to
keep satisfying that chain, or those users stay on 0.21.1 forever. So the feed's shape, the signing
key, the installer's identity and the asset names are all the Tauri app's.
`.github/workflows/upgrade-test.yml` builds v0.21.1 from its tag and watches its real updater
install the current app on both platforms.

## The feed and the key

- `crates/update` reads `latest.json` (`{ version, notes, pub_date, platforms }`, written by
  `scripts/ci-build-latest.mjs`) and verifies each download with the same public key, byte for
  byte.
- CI signs with the `TAURI_SIGNING_PRIVATE_KEY` secret through `npx @tauri-apps/cli signer sign`
  (the key format is Tauri's) and checks every signature with
  `cargo run -p hyprspace-update --example verify` before uploading. No machine needs the key.
- **The feed and key can only change at compile time** (`HYPRSPACE_UPDATE_FEED`,
  `HYPRSPACE_UPDATE_PUBKEY` through `option_env!`). Read at run time, anyone who could set a
  variable for the app could feed it an installer. Release CI never sets them.
- **Versions only go up.** The updater compares numbers, so a flat or lower one means "no update".

## The app's updater

- **Only an installed copy updates.** A copy next to the installer's `uninstall.exe`, or inside a
  `.app`, is installed; `cargo run` and `target/release` builds answer `Unmanaged` and never check,
  so a dev build can't install a release over the user's app.
- It checks on launch, every 6 hours and on focus after 15 minutes, silently on failure, and shows
  a corner card with "Restart and update". An install checks the feed again first, so an app
  several releases behind lands on the newest in one hop. What's new reads the `docs/CHANGELOG.md`
  built into the app on the first launch of a new version.
- **Windows:** the engine starts the NSIS installer with the Tauri updater's own arguments
  (`/P /R /UPDATE /ARGS`) and the UI quits.
- **macOS:** the engine unpacks the `.app.tar.gz` over the bundle, and a detached shell reopens it
  once this process is gone.
- Both updaters stage the installer in a temp folder and quit before they could delete it, so an
  installed copy sweeps ours and the Tauri updater's leftovers at launch, retrying while the
  installer that just ran still holds its file.

## Installers

- **Windows** (`apps/hyprspace/package/windows/installer.nsi`, built by
  `scripts/package-windows.ps1`) writes the per-user registry keys and folder the Tauri installer
  wrote, so Apps keeps one HyprSpace entry. It deletes the old `hyprspace-tauri.exe` and points the
  Start menu, desktop and pinned taskbar shortcuts at `hyprspace.exe`. Shortcuts carry the
  AppUserModelID `com.hyprspace.app`, which the app sets too, so its window groups with a pinned
  HyprSpace.
- **It closes only its own copy.** It finds processes whose image is in its install folder, sends
  their window `WM_CLOSE` so the app kills its PTYs the normal way, and kills only what's left 5 s
  later. Matching by name would close every HyprSpace on the machine. The installer is 32-bit and
  can't read a 64-bit process's path, so it runs the 64-bit PowerShell through `Sysnative`.
- `package-windows.ps1 -Test` builds "HyprSpace Test" with its own name, keys, folder and
  AppUserModelID, so it can be installed, updated and removed beside the real app.
  `scripts/check-windows-install.ps1` runs those checks.
- **macOS** (`scripts/package-macos.sh`) assembles `HyprSpace.app` (`com.hyprspace.app`, executable
  `hyprspace`), the `.app.tar.gz` and a dmg. It signs with the Developer ID and notarizes when the
  Apple secrets exist, otherwise signs ad hoc, which Apple silicon needs to launch it at all.
- The uninstaller never touches user data (`~/.hyprspace`).
