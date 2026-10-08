# Release

Only ship when Ash asks.

```powershell
.\deploy.ps1 patch "One bullet per line`nAnother bullet"
```

Run it from a clean `main`. It bumps the workspace `version` in `Cargo.toml` and the workspace
entries in `Cargo.lock`, writes the notes into `docs/CHANGELOG.md` (which the app bundles for
What's new), commits `release: v<new>`, tags, pushes both, opens a draft GitHub release with the
notes and starts `.github/workflows/release.yml`. CI builds Windows and macOS with
`scripts/package-*`, signs what the updaters install, merges each into `latest.json`, and publishes
the draft when both are in.

A dry run touches no release: `gh workflow run release.yml --ref <branch> -f dry_run=true`.

## Notes

Written at ship time from `git log <lastTag>..HEAD`: a few short bullets a user cares about, in
plain English. Bullets are split on newlines or ` | `.

## Which digit

- **patch** for fixes, polish and one or two small additions. This is the default; Ash picks the
  bump, so say which one before running `deploy.ps1`.
- **minor** for a real batch of new features, or a change in how existing things work.
- **major** is saved for 1.0.

Versions only ever go up: the updater treats a flat or lower number as "no update", and release CI
refuses a tag that doesn't match the workspace version. The app continued the Tauri app's numbers
from 0.21.1, so the installed Tauri app sees each release as newer.

## If a run stops halfway

`deploy.ps1` isn't idempotent. Before running it again, check `git log`, local and remote tags
(`git ls-remote --tags origin`), `gh release list` and `gh run list`. If the workspace version is
already ahead of the last published release, finish or clean up by hand rather than bumping again.
Don't move or delete a pushed tag without asking.

## The Android app

The app has its own version in `mobile/VERSION` and ships on its own, separate from the desktop:

```powershell
.\mobile\deploy.ps1 patch "One bullet per line`nAnother bullet"
```

It bumps `mobile/VERSION`, commits `android: android-v<new>`, tags, pushes, opens a draft release
and runs `.github/workflows/android.yml`. CI tests and builds the APK, signs it with the
`ANDROID_KEYSTORE_BASE64` and `ANDROID_KEYSTORE_PASSWORD` secrets (key alias `hyprspace`),
attaches `HyprSpace-android-<version>.apk` and `HyprSpace-android.apk`, and publishes the release
with `--latest=false`. The repo's latest release stays the desktop's, because the desktop updater
reads `latest.json` from it; `release.yml` marks each desktop release latest when it publishes.
The same run replaces the APK on `android-latest`, a standing pre-release whose fixed link is the
QR code in the desktop's Settings, Phone. The app's updater skips it.
`versionCode` is worked out from the version (major * 1000000 + minor * 1000 + patch), so it only
goes up, as Android needs.

Installed apps list the releases every six hours, take the newest `android-v` one with an APK,
check its SHA-256 against GitHub's digest and its signer against their own, and install it with
Android's package installer. The first update asks; after that, on Android 12 and newer, the app
installs updates itself a minute after it leaves the screen.

The app and the desktop stay compatible through `PROTOCOL`, not their versions. A phone feature
that needs something new from the desktop ships both, and the app hides it until the desktop says
it is new enough (`Link.desktopAtLeast`, against the version in `Welcome`).

A desktop release that changes `PROTOCOL` needs an app release with it: a phone on the old
protocol is told to update, not let in.
