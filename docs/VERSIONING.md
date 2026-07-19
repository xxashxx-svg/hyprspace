# Versioning

HyprSpace uses **adapted Semantic Versioning**: `MAJOR.MINOR.PATCH` (e.g. `0.2.5`). Every release
bumps the version through the maintainer's release tooling — the three version files
(`tauri.conf.json`, `package.json`, `Cargo.toml`) always move together.

```
   0   .   2   .   5
 MAJOR   MINOR   PATCH
```

## What each digit means

| Digit | Bump when… | Example | deploy.ps1 |
|---|---|---|---|
| **PATCH** | bug fix, perf, polish, copy/UI tweak — nothing new to learn | `0.2.5 → 0.2.6` | `.\deploy.ps1 patch "…"` |
| **MINOR** | a new user-facing feature (backward compatible). Patch resets to 0 | `0.2.x → 0.3.0` | `.\deploy.ps1 minor "…"` |
| **MAJOR** | a milestone (the 1.0 launch) or, post‑1.0, a big/breaking redesign | `0.x → 1.0`, `1.x → 2.0` | `.\deploy.ps1 major "…"` |

**Decision flow each release:**
1. Did something break or change how existing things work? → pre‑1.0: **minor**; post‑1.0: **major**.
2. Else, is there a new feature? → **minor**.
3. Else (only fixes / tweaks)? → **patch**.

Rule of thumb: *"Would a user notice and need to care?"* No → patch. New thing they can use →
minor. Big shift / it's a new era → major.

## We're pre‑1.0 (`0.x`) — what that means

`0.x` officially means "still taking shape, anything can change." While we're below 1.0:
- **patch** = fixes, **minor** = features *and* the occasional "it works differently now" change
  (pre‑1.0 you're allowed to break things in a minor).
- **`1.0.0` is a statement**, not just a number — "we consider this stable and ready to rely on."
  Save the major bump for that launch moment. After 1.0, *then* major = big/breaking redesign.

## Hard rules (the updater depends on these)

- **Always increase.** The Tauri updater compares versions; a flat or lower number means "no
  update." Never reuse or roll back a version.
- **One bump per release.** Don't hand-edit version fields — `deploy.ps1` updates all three
  (`tauri.conf.json`, `package.json`, `Cargo.toml`), tags `v<new>`, and writes the manifest with
  the same number, so the in‑app version, git tag, and update feed can't drift.

## How this compares to the wider world

- **Libraries / packages** (React, npm, crates) use *strict* SemVer — break the public API → major.
- **Apps with no API** (VS Code, Slack — and us) adapt it: VS Code ships **minor ≈ monthly feature
  drops**, **patch for hotfixes**, **major almost never**. Chrome time-boxes its major. CalVer
  (Ubuntu `24.04`, JetBrains `2024.1`) is date-based — not a fit here since we ship when ready and
  the updater wants meaningful increments. Adapted SemVer (above) is our model.

## Examples from this codebase

- cwd resume fix, audit fixes, status-bar restyle → **patch**
- Loops, the multi-agent launcher, the integrated editor, Skills, Codex provider → **minor**
  (a batch of these together is one minor, e.g. `0.2.x → 0.3.0`)
- declaring HyprSpace stable / "v1" → **major** (`→ 1.0.0`)
