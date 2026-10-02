# ADR 0011: The GPUI app updates from the Tauri app's feed and key

- Status: Accepted
- Date: 2026-10-02

## Context

The Tauri app reads `releases/latest/download/latest.json` and checks each download against the
minisign key in `src-tauri/tauri.conf.json`. The GPUI app needs its own updater, and the first
GPUI release is installed by the Tauri one.

## Decision

**One feed, one key, one shape.** `crates/update` reads the same `latest.json`
(`{ version, notes, pub_date, platforms }`) with the same platform keys and verifies with the same
public key, byte for byte. CI keeps signing with the `TAURI_SIGNING_PRIVATE_KEY` secret, now
through `npx @tauri-apps/cli signer sign` instead of tauri-action, and checks each signature
against the app's key (`cargo run -p hyprspace-update --example verify`) before anything is
uploaded.

**The engine does the work, the UI asks.** `UpdateCommand::{Check, Install}` and `UpdateEvent`
cross the channel (`proto/src/update.rs`); `engine/src/update.rs` downloads, verifies, stages and
installs. An install checks the feed again first, so an app several releases behind lands on the
newest in one hop, as the Tauri store did. On Windows it starts the installer with Tauri's
arguments (ADR 0010) and the UI quits: the installer waits for the exit and starts the new
version. On macOS it swaps the bundle in place and a detached shell opens it once this process
is gone (zeron's `relaunch_after_exit`).

**Only an installed copy replaces itself.** A copy whose folder has the installer's
`uninstall.exe`, or that runs from inside a `.app`, is installed; anything else (`cargo run`,
`target/release`) answers `Unmanaged` and never checks. A dev build can't install a release over
the user's app.

**The UI follows the Tauri app's** `stores/updater.ts`, `Updater.tsx` and General card: a quiet
check on launch, every 6 hours and on focus after 15 minutes, silent when it fails; a corner card
with "Restart and update", progress, and Retry; the same state in Settings, General. What's new
reads the changelog built into the app, after `WhatsNew.tsx`, and shows on the first launch whose
version differs from `AppState.seen_version`. The Tauri app's `lastSeenVersion` comes over with
its state (ADR 0012), so the first GPUI launch after the update says what changed.

**Downloads don't pile up in the temp folder.** Both updaters stage the installer in a temp
folder and quit before they could delete it (the installer runs from there). So an installed copy
clears them at launch (`hyprspace_update::sweep` from `Engine::start`): our `hyprspace-update-*`
folders and the Tauri updater's `HyprSpace-<version>-updater-*` ones. Right after an update the
installer that started us may still hold its file, so a busy folder gets a few more tries.

**A test feed and key are a build-time switch.** `HYPRSPACE_UPDATE_FEED` and
`HYPRSPACE_UPDATE_PUBKEY` are read with `option_env!` while the crate compiles, never at run time,
so a shipped build can't be pointed elsewhere. Release CI never sets them. The end-to-end check
(REWRITE.md, Phase 6c) built two versions against a local feed and a throwaway key.

## Rejected

- **Reading the feed or key from the environment at run time.** Anyone who could set a variable
  for the app could feed it an installer.
- **Installing on quit, as zeron does.** The Tauri app asks first and restarts right away; the
  same habit is less surprising for people who just updated from it.
