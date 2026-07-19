# Contributing to HyprSpace

Thanks for taking a look. This is a Tauri 2 + React app — the frontend is TypeScript/Vite, the
backend is Rust. Everything below assumes you've cloned the repo.

## Dev setup

Prerequisites:

- **Rust** (stable) via [rustup](https://rustup.rs) — `edition = "2021"`, current stable is fine.
- **Node 18+** (20 or 22 recommended) and npm.
- **Windows:** the MSVC C++ build tools (Visual Studio Build Tools with "Desktop development with
  C++") and the **WebView2** runtime — already present on Windows 10/11 in most cases.
- **macOS:** `xcode-select --install`.
- **Linux:** the usual Tauri deps (`webkit2gtk-4.1`, `libsoup-3.0`, `librsvg2`, `build-essential`).
  Linux mostly works but isn't shipped.

Then:

```bash
npm install
cp .env.example .env      # optional — see below
npm run tauri dev
```

`.env` holds `VITE_SUPABASE_URL` / `VITE_SUPABASE_ANON_KEY`. Supabase is used for sign-in only, and
auth fails open in dev — the app runs fine without them, you just won't have an account.

To actually launch agents you need whichever CLI you want to use already installed and logged in
(`claude`, `gemini`, `codex`). HyprSpace spawns them; it never authenticates on their behalf.

## Repo layout

`CLAUDE.md` has the full repo map and an architecture overview — read that first. Short version:

- `src/` — React frontend. `components/` UI, `stores/` Zustand state, `api/index.ts` the typed
  bridge over Tauri `invoke()`, `styles/` per-area CSS.
- `src-tauri/src/` — Rust. `lib.rs` registers every command, `pty.rs` is the terminal backend,
  `devtools/` is the git/fs/project command surface, `persist.rs` is the state store.
- `docs/` — deeper material, starting at [docs/README.md](./docs/README.md).
- `website/` — the marketing site, a separate Vite app ([website/README.md](./website/README.md)).

## Code style

These are hard rules, not preferences:

- **Vanilla CSS + design tokens only.** Use the variables in `src/styles/tokens.css`
  (`--surface-*`, `--text-*`, `--border-*`, `--accent`, `--status-*`). **No Tailwind, no CSS-in-JS**
  in the app (the `website/` sub-project is Tailwind — that's separate). Match the existing neutral,
  low-contrast dark look.
- **No `React.StrictMode`.** It's deliberately off in `main.tsx`; double-mounting corrupts xterm.js
  lifecycles. Don't re-add it.
- **Don't churn `TerminalPane`.** Avoid patterns that frequently unmount/remount it, and always
  dispose xterm instances and addons on cleanup — orphaned ConPTY hosts burn CPU.
- **Components call `src/api/index.ts`**, never `invoke()` directly. Filesystem-heavy Tauri commands
  are `async fn` + `spawn_blocking` — sync commands run on the UI thread.
- **Comments are minimal and casual.** Comment tricky logic only, lowercase, short. Match the
  surrounding code rather than your own preferred style.
- **Never hand-edit version numbers.** The `version` in `src-tauri/tauri.conf.json`, `package.json`
  and `src-tauri/Cargo.toml` is managed exclusively by `deploy.ps1`. A PR that bumps them will be
  asked to revert. See [docs/VERSIONING.md](./docs/VERSIONING.md).

One more thing worth knowing: HyprSpace runs Claude on the user's own subscription by spawning their
already-logged-in `claude` CLI. Don't add a custom claude.ai OAuth flow, and don't read or forward a
subscription token to call the Anthropic API directly. Reading credential files for display-only
fields (email, plan) is fine.

## Checks before you push

```bash
npx tsc --noEmit              # typecheck the frontend
cd src-tauri && cargo check    # typecheck the backend
```

There's no test suite yet. Run the app (`npm run tauri dev`) and exercise the thing you changed —
TS hot-reloads, Rust changes trigger a recompile and relaunch.

## Opening a PR

1. Branch off `main`.
2. Keep the diff scoped to one thing. Don't reformat or "improve" adjacent code.
3. Run the two checks above.
4. Open the PR against `main` and fill in the template — what changed, why, and how you verified it.
   Screenshots help for UI changes.

For anything large or architectural, open an issue first so we can agree on the shape before you
write it.

## Reporting bugs and vulnerabilities

Bugs → GitHub issues, using the bug report template. Security issues → **not** an issue; see
[SECURITY.md](./SECURITY.md).
