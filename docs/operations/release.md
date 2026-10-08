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

`mobile/` has its own version, `hyprspace.version` in `mobile/gradle.properties`, and its
`versionCode` is worked out from it. Raise it by hand when the app changes; it only goes up,
since Android refuses an update whose `versionCode` isn't higher.

The app ships on a desktop release, only when asked: run `release.yml` with **android** ticked
(or ask for it alongside a `deploy.ps1` release). The job tests and builds the APK and attaches
it as `HyprSpace-android-<version>.apk` and `HyprSpace-android.apk`. It signs with the
`ANDROID_KEYSTORE_BASE64` and `ANDROID_KEYSTORE_PASSWORD` secrets (key alias `hyprspace`); without
them the APK is signed with a throwaway key and can't update an install of a real release.

A desktop release that changes `PROTOCOL` needs an app release with it: a phone on the old
protocol is told to update, not let in.
