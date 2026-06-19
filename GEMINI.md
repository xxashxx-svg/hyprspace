# HyprSpace — agent guide

> **Canonical guide: [CLAUDE.md](./CLAUDE.md)** (and [docs/](./docs/README.md)). This file mirrors
> the essentials for any agent; read CLAUDE.md for the full picture and keep both in sync.

A multi-terminal AI workspace built with **Tauri 2 + React**. It tiles Claude Code / Gemini / Codex
/ shell sessions across **projects** and **open spaces**, with per-pane resume, drag-to-swap, a
command palette, a git review dock, a home-screen AI chat that can operate the app, and live
auto-update.

## Tech stack
- **Frontend:** React 19, Vite, TypeScript, Zustand. Vanilla CSS with design tokens (no Tailwind).
- **Backend:** Tauri 2, Rust.
- **Terminal:** `xterm.js` (WebGL/Search/Fit) over `portable-pty` (Rust) with coalesced byte streaming.
- **Persistence:** crash-safe JSON store in Rust (`persist.rs`). Supabase for the app's own sign-in only.
- **AI:** spawns the user's logged-in CLIs (Claude/Gemini/Codex). The home chat drives a persistent
  `claude` stream-json process.

## ⚠️ Critical constraints (full list in CLAUDE.md)
1. **Subscription compliance:** run Claude only by spawning the user's already-logged-in `claude`
   CLI. NEVER build a custom claude.ai OAuth flow, read/forward the subscription token for auth, or
   feed it to an SDK. Reading credential files for display-only fields (email/plan) is fine.
2. **No `React.StrictMode`** — disabled in `main.tsx` to protect xterm.js lifecycles.
3. **Styling = design tokens** (`src/styles/tokens.css`) — no Tailwind/CSS-in-JS.
4. **Terminal stability** — don't cause frequent unmount/remount of `TerminalPane`; dispose xterm
   on cleanup; PTYs are killed on exit (`kill_all`).
5. **Versions via `deploy.ps1` only** — never hand-edit `tauri.conf.json`/`package.json`/`Cargo.toml`
   versions. See [docs/VERSIONING.md](./docs/VERSIONING.md).
6. **Don't commit/push/deploy unless asked.** Code style: human/casual, minimal comments.

## Architecture (conventions)
- **State:** Zustand stores in `src/stores/`; hydration/persistence handled in `App.tsx`.
- **IPC:** components call typed wrappers in `src/api/index.ts`, never `invoke()` directly. All Rust
  commands are registered in `src-tauri/src/lib.rs`. Filesystem-heavy commands are `async fn` +
  `spawn_blocking` so the UI never freezes.
- **PTY:** `PtyManager` (`src-tauri/src/pty.rs`) handles terminal lifecycle + byte coalescing. Panes
  run a bare shell; launch commands are typed in as keystrokes (not argv).
- **Home chat / orchestrator:** `chat.rs` + `stores/chat.ts` (persistent stream-json session);
  `stores/orchestrator.ts` parses the model's ```hyprspace blocks to operate the app. Details in
  [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md).

## Build / run / deploy
```bash
npm install
npm run tauri dev          # dev with HMR
npm run tauri build        # production build
```
```powershell
.\deploy.ps1 patch "description of changes"   # bump + build + sign + publish (see docs/DEPLOY.md)
```
Signing keys live in `~/.hyprspace-signing`; releases go to the public `hyprspace-releases` repo.

## Guidelines
- Maintain per-thread/per-session resume correctness: `claude --resume <id>` is **folder-scoped** —
  resume only works in the directory the session was created in.
- Handle PTY/persistence errors gracefully on both Rust and TS sides.
- Use design tokens for styling; match the neutral, low-contrast look.
