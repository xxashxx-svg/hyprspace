# AGENTS.md — working on HyprSpace alongside other agents

This file is for **AI agents (and humans) working on this repo in parallel**. It's about
*coordination*: where to work so you don't collide, which files are shared hot-spots, and how to
verify your change. For what the app **is** and how it works, read **[CLAUDE.md](./CLAUDE.md)** first —
that's the canonical guide (architecture, critical constraints, repo map, deploy runbook).

---

## Where things live (the UI is all under `src/`)

```
src/                     the entire frontend (React + TS) — "the UI and stuff" is here
  App.tsx                app shell / layout
  App.css                ORDERED @import index of styles/*.css — don't add rules here
  styles/tokens.css      design tokens (theme variables)
  styles/<area>.css      per-area component CSS: rail, home, pane, loops, launcher, editor,
                         chat, settings, files-tree, services, command-palette, titlebar, … (~30 files)
  components/*.tsx        one file per UI component (Rail, PaneGrid, TerminalPane, HomePage,
                         ChatPanel, ReviewDock, Settings, LoopsPage/Manager, LaunchWorkspace,
                         CodeEditor, Titlebar, CommandPalette, …)
  stores/*.ts            Zustand state, one file per domain (workspace, ui, settings, chat, loops,
                         launchPresets, git, services, …)
  lib/*.ts               small helpers + engines (loops.ts engine, loopTemplates, names, projects, time)
  api/index.ts           typed bridge over Tauri invoke()/Channel — components import THIS, never invoke()

src-tauri/               the Rust backend
  src/lib.rs             all #[tauri::command] registrations + app lifecycle
  src/<feature>.rs       pty, chat, agent, services, persist, license, oauth, ai
  src/devtools/          dev-cockpit commands split per area: git.rs, worktree.rs, project.rs,
                         fs.rs, providers.rs, mcp.rs, skills.rs (+ mod.rs re-exports & shared helpers)
  tauri.conf.json        app config / version / updater / capabilities

scripts/logo.svg         the brand mark SOURCE (cube). Regenerate app icons:
                         node scripts/gen-icon.mjs && npx tauri icon scripts/logo-1024.png
website/                 the marketing landing page (static, deployed on Vercel)
```

The codebase is **already split so parallel agents rarely overlap**: components, stores, helpers, and
CSS are all per-area, and `devtools.rs` is split per domain. Pick a lane and you'll mostly touch your
own files.

---

## Pick a lane

- Building/altering a UI feature → its `components/<X>.tsx` + `styles/<x>.css` (+ maybe its store).
- New CSS area → add `styles/<x>.css` **and** one `@import` line in `App.css` (keep it in visual order).
- State change → `stores/<domain>.ts`.
- Backend command → the right `src-tauri/src/<feature>.rs` or `src/devtools/<area>.rs`.

## Shared hot-files — coordinate / serialize edits here

These are touched by *many* features; expect to merge, and edit at the documented spots:

- **`src-tauri/src/lib.rs`** — the command registry. Every new `#[tauri::command]` is registered here.
- **`src/api/index.ts`** — every new command needs a typed wrapper here.
- **`src/stores/ui.ts`** — shared UI state (views, dock, dialogs); many features add a flag.
- **`src/App.css`** — only changes when you add a new `styles/*.css` file (one line).
- **`tauri.conf.json` / `package.json` / `Cargo.toml`** — **versions are managed ONLY by `deploy.ps1`.**
  Never hand-edit a version number.

## The claim board (when running inside HyprSpace)

If `HYPRSPACE_SESSION_ID` is set, you're in a shared HyprSpace space with other agents. **Before editing
a set of files**, use the `hyprspace` MCP claim board: `check_files` → `claim_files` (owner = your
session id). If a claim conflicts, **don't edit those files** — pick other work or coordinate.
`release_files` when done. A PreToolUse hook also auto-claims and blocks conflicting edits, so claim
proactively rather than getting blocked mid-edit.

---

## Critical constraints (do NOT violate — full detail in CLAUDE.md)

1. **Subscription compliance.** The app runs Claude by spawning the user's already-logged-in `claude`
   CLI. **Never** use the subscription OAuth token as an API key, feed it to an SDK, or build a
   claude.ai OAuth flow. (Loops are the exception that's still compliant: they use a **separate**
   Anthropic API key from the OS keychain — never the subscription.)
2. **No `React.StrictMode`** (double-mcount corrupts xterm.js).
3. **Vanilla CSS + tokens only** — no Tailwind, no CSS-in-JS. Use the variables in `styles/tokens.css`.
4. **Terminal stability** — don't add patterns that unmount/remount `TerminalPane`; dispose xterm +
   addons on cleanup.
5. **Versions via `deploy.ps1` only.**
6. **Don't commit / push / deploy unless explicitly asked.** Work in dev mode (HMR).
7. **Code style:** human/casual, minimal comments (only tricky logic, short and lowercase).

---

## Verify before you hand off

- **Frontend types:** `npx tsc --noEmit`
- **Frontend bundle/imports:** `npm run build`
- **Rust:** `cargo check --manifest-path src-tauri/Cargo.toml`
- **Run / preview:** `npm run tauri dev`. To run a dev instance **without touching a user's real data**,
  set `HYPRSPACE_STATE_DIR=<a scratch dir>` (a dev-only override in `persist.rs`) so it uses isolated
  state instead of `~/.hyprspace/v2`.

CSS changes hot-reload; Rust changes trigger a recompile + relaunch. A `<task>` is done only when it
typechecks (and compiles, if Rust changed).
