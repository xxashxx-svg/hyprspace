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
- A check that finds a release downloads and verifies it in the background and keeps it in
  memory, so the click goes straight to installing. With threads still running, the first click
  says how many will stop and the second installs.
- Handing over writes `update-pending` in the state folder. A launch that is still on the old
  version reads it and shows "The update to x didn't finish" with Retry.
- **Windows:** the engine starts the NSIS installer with the Tauri updater's arguments, silent
  instead of passive (`/S /R /UPDATE /ARGS`), and the UI quits. The app closes and reopens on the
  new version with no installer window, the way other desktop apps update.
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
- **It also matches on the command line.** A claude that dies while starting a hook or status
  line leaves that `hyprspace.exe` suspended for good, with no path to read, holding the exe open.
  The app ends such strays at launch and before an update (`hooks::end_strays`), and the installer
  catches any it missed.
- **An update that fails reopens the old version** (`.onInstFailed`), so the user isn't left
  without the app.
- `package-windows.ps1 -Test` builds "HyprSpace Test" with its own name, keys, folder and
  AppUserModelID, so it can be installed, updated and removed beside the real app.
  `scripts/check-windows-install.ps1` runs those checks.
- **macOS** (`scripts/package-macos.sh`) assembles `HyprSpace.app` (`com.hyprspace.app`, executable
  `hyprspace`), the `.app.tar.gz` and a dmg. It signs with the Developer ID and notarizes when the
  Apple secrets exist. Without them it signs with the self-signed certificate in the
  `MACOS_SIGNING_CERTIFICATE` secrets, and with neither, ad hoc, which Apple silicon needs to
  launch it at all.
- **macOS permissions follow the signature.** A granted permission (Accessibility, Screen
  Recording) is kept against the app's designated requirement. Signed ad hoc, that's the build's
  own hash, so every update looks like a new app and the grant silently stops working while
  System Settings still shows it on. A certificate makes it the bundle id and that certificate,
  which an update keeps. So the certificate must never change: a new one asks every Mac user again.
  `scripts/make-macos-signing-cert.sh` makes it once and keeps a copy in `~/.hyprspace-signing`.
- The uninstaller never touches user data (`~/.hyprspace`).

## Agent updates

HyprSpace also updates Claude and Codex, the way T3 Code does, with each CLI's own updater
(`engine/src/agent_update.rs`). Settings, Agents shows each one's install and update; the corner
card and the sidebar's update button offer what's behind; "Restart and update" updates the agents
first. Only this computer's agents, and only on a click.

- **It updates the file the harness starts.** `<cli>.exe` first on PATH, then the shim `cmd`
  finds, resolved through links and junctions. How it was installed comes from that path:
  `~/.local/bin/claude` is Claude's own installer (`claude update`), a release under
  `.codex/packages/standalone` is Codex's (`codex update`, with that `.codex` as `CODEX_HOME`),
  npm only when `npm prefix -g` holds both the command and the package, Homebrew from the keg.
  Anything else (WinGet, bun, pnpm, a project's `node_modules`) shows the command instead of
  running it: WinGet's Claude answers `claude update` with success and stays as it was.
- **A running binary is fine.** `claude update` renames the locked `claude.exe` aside and writes
  the new one; `codex update` installs a new release folder and repoints its junction. Running
  sessions keep their version until they next start, so nothing is closed for an update. Tested on
  Windows in a scratch home with a session holding each binary.
- **Updaters run as plain children**: no terminal, no stdin, never typed into (the Codex
  update dialog that ate an Enter is why). A failure shows the last line the updater printed,
  which is where they put the reason, minus any crash backtrace. Success is the version moving; a
  run that leaves it where it was says "already the newest on its channel".
- **A slow updater is never killed.** Stopping `claude update` while it copies the new
  `claude.exe` into place would leave no working Claude. After 15 minutes HyprSpace says it
  stopped waiting, keeps the agent's lock until the updater exits, then reports where the version
  ended up. HyprSpace puts no job object on its children, so an updater also outlives the app.
- **npm on Windows waits for its package to be free.** npm can't replace files a running process
  holds and stops halfway, so with anything running from the package's folder the update refuses
  and says to close the agent's threads. The native installers cope with a running binary;
  npm doesn't.
- **System32 goes first on the updater's PATH.** Codex's installer runs `tar`, and Git's GNU tar
  ahead of Windows' own reads `C:` as a remote host and the update fails.
- **The newest version comes from npm**, alongside the app's own update check, behind Settings,
  General, "Check for agent updates". The switch is read from saved state before the first
  check, so a user who turned it off never sends one. npm only knows Claude's `latest` channel,
  so a Claude on `stable` isn't called behind.
- **Bad versions are held back** through `avoid_agents` in `latest.json`
  (`scripts/avoid-agents.json`). Native Claude can go back with `claude install <version>`.
- **Codex terminal threads start with `check_for_update_on_startup=false`**, so Codex's own
  update prompt never opens under a thread.
