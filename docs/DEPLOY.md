# Releasing HyprSpace

How to ship a new version and push an auto-update — **written so you can do it without Claude.**

## The release model (why it's split)

- **Windows** is built **locally** on your machine by `deploy.ps1` (free, fast, you have the
  toolchain). This is the source of truth for a release — it creates the GitHub release and the
  Windows half of the update manifest.
- **macOS** can only be built on a Mac, so it's built by **GitHub Actions CI**
  (`.github/workflows/release.yml`) on a macOS runner. `deploy.ps1` triggers it automatically after
  publishing; CI builds + signs the Mac app and **merges** the darwin entry into the same
  release's `latest.json` (keeping the Windows entry).

Two repos:
| Repo | Visibility | Role |
|---|---|---|
| `xxashxx-svg/hyprspace-2` | private | the **source code** (this repo) + the macOS CI workflow |
| `xxashxx-svg/hyprspace-releases` | public | published **releases** + the `latest.json` update feed |

Installed apps check `hyprspace-releases`'s latest `latest.json` on launch (Tauri updater) and
update themselves. The manifest is **minisign-signed**, so only builds signed with the private key
are accepted.

---

## One-time setup

You need these once on the machine you release from:

1. **Toolchain:** Node 20+ and Rust (stable). `npm install` in the repo.
2. **GitHub CLI authed:** `gh auth login` as an account with push access to **both** repos.
   (`deploy.ps1` uses `gh release create` on `hyprspace-releases` and `gh workflow run` on
   `hyprspace-2`.)
3. **Signing keys** in `~/.hyprspace-signing/` (i.e. `C:\Users\<you>\.hyprspace-signing\`):
   - `hyprspace.key` — the minisign **private** key (Tauri updater signing key).
   - `password.txt` — its password.
   These are **secret, never committed**. If you don't have them, get them from Ansh (they were
   generated with `npm run tauri signer generate`). Without them you can't sign an update.
4. **CI secrets** (already set on the `hyprspace-2` repo, only needed if re-creating CI):
   `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, `RELEASES_TOKEN`
   (a PAT that can publish to `hyprspace-releases`).

---

## Shipping a release

From the repo root, in **PowerShell**:

```powershell
.\deploy.ps1 patch "Short description of what changed"
```

Pick the bump level by the rules in [VERSIONING.md](./VERSIONING.md):
- `patch` — bug fixes / polish (`0.2.5 → 0.2.6`)
- `minor` — new features (`0.2.x → 0.3.0`)
- `major` — milestone / 1.0 / big redesign (`0.x → 1.0`)
- `none` — publish the **current** version as-is without bumping (rare; e.g. re-publishing)

### What `deploy.ps1` does, in order
1. Reads the current version from `tauri.conf.json` and computes the new one.
2. Writes the new version into **all three** files: `src-tauri/tauri.conf.json`, `package.json`,
   `src-tauri/Cargo.toml`.
3. Loads the signing key/password from `~/.hyprspace-signing` into env.
4. `npm run tauri build` → builds + signs the **NSIS** installer
   (`src-tauri/target/release/bundle/nsis/*-setup.exe` + `.sig`). Takes a few minutes.
5. Writes `release-artifacts/latest.json` (the update manifest: version, notes, date, and the
   Windows `signature` + download `url`).
6. If the bump isn't `none`: `git add -A`, commits `release v<new>`, and `git push` (to
   `hyprspace-2`).
7. `gh release create v<new> --repo hyprspace-releases …` — publishes the release with the
   installer + `latest.json`.
8. `gh workflow run release.yml --repo hyprspace-2 -f tag=v<new>` — kicks the macOS CI build, which
   adds the darwin entry to the manifest a few minutes later.

When it finishes, the Windows release is live immediately; the macOS half lands when CI completes
(~10–15 min).

---

## macOS build (CI)

`deploy.ps1` triggers it for you. To run it **manually** (e.g. CI didn't fire, or you re-ran a
release):

```bash
gh workflow run release.yml --repo xxashxx-svg/hyprspace-2 --ref main -f tag=v0.3.0
```

It builds + signs the Apple-Silicon app, then merges the darwin entry into that release's
`latest.json` (preserving the Windows entry) and uploads it. Watch it at the repo's **Actions** tab
or `gh run watch`.

> Local Mac build (no CI, just to get a `.dmg` for yourself): see [../BUILD-MAC.md](../BUILD-MAC.md).

---

## Troubleshooting

- **"signing key not found at …\.hyprspace-signing\hyprspace.key"** — you're missing the signing
  keys. Get them from Ansh and put `hyprspace.key` + `password.txt` in `~/.hyprspace-signing`.
- **`gh release create` fails / 401/403** — `gh auth login` and make sure the account can publish
  to `hyprspace-releases`.
- **`npm run tauri build` fails** — run `npm run tauri build` on its own to see the full error;
  common causes are a Rust compile error (`cargo check` in `src-tauri/`) or a TS error
  (`npx tsc --noEmit`). Fix, then re-run `deploy.ps1`.
- **Installer/signature not found** — the NSIS bundle didn't build; check that
  `createUpdaterArtifacts: true` in `tauri.conf.json` and the build actually produced
  `target/release/bundle/nsis/*-setup.exe`.
- **macOS CI didn't trigger** — run it manually (command above). The release on
  `hyprspace-releases` already exists from the Windows step; CI just adds the Mac files.
- **Version looks wrong in the app** — the version is read from `tauri.conf.json` at **build** time
  (not hardcoded). In dev it shows the last-compiled value; the shipped installer always has the
  bumped number. Never hand-edit version fields — let `deploy.ps1` do it.

## Quick checklist (no-Claude version)
1. `git status` clean-ish, on `main`, changes are what you want to ship.
2. `npx tsc --noEmit` passes; app runs in `npm run tauri dev`.
3. Decide bump level (VERSIONING.md).
4. `.\deploy.ps1 <patch|minor|major> "what changed"`.
5. Confirm the release appears on `hyprspace-releases` with the `*-setup.exe` + `latest.json`.
6. Wait for macOS CI (or trigger manually); confirm the `.dmg` + updated `latest.json` land.
7. Done — installed apps will self-update on next launch.
