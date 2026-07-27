# HyprSpace — project guide

> Read this first. It's the canonical guide for anyone (human or AI agent) working on this repo.
> Deep dives live in [`docs/`](./docs/README.md).

HyprSpace is a **multi-terminal AI workspace** — a Tauri 2 + React desktop app that tiles Claude
Code / Gemini / Codex / shell sessions across **projects** and **open spaces**, with per-pane
resume, drag-to-swap, a multi-agent **launcher** (fan out N agents in a folder at once), scheduled
**Automations**, an integrated **code editor**, a command palette, and a git review dock. Neutral,
T3-Code-inspired dark UI.

- **Stack:** Tauri 2 (Rust) · React 19 + TypeScript + Vite · Zustand state · xterm.js (WebGL) ·
  `portable-pty` (Rust) · Supabase (auth only) · auto-update via Tauri updater + minisign.
- **Platforms:** Windows (primary, built locally) + macOS (built in CI). Linux mostly works but isn't shipped.
- **Companion app:** [`mobile/`](./mobile/README.md) — an Expo/React Native Android app that pairs
  over your LAN and mirrors spaces, panes and live terminals. Its own app, its own versioning.

---

## ⚠️ Critical constraints — do not violate

1. **Subscription compliance (most important).** The app runs Claude on the user's **subscription**
   by spawning their already-logged-in `claude` CLI — nothing else. **NEVER** build a custom
   claude.ai OAuth flow, **NEVER** read/store/forward the subscription OAuth token to call the
   Anthropic API yourself, **NEVER** feed subscription tokens to an SDK. Reading credential files
   for **display-only** fields (email / plan) is fine; using a token for auth is not. This is the
   same sanctioned path T3 Code uses. The terminal panes just spawn the CLI.
2. **No `React.StrictMode`.** It's intentionally disabled in `main.tsx` — double-mounting corrupts
   xterm.js lifecycles. Don't re-add it.
3. **Styling = vanilla CSS + design tokens.** Use the CSS variables in `src/styles/tokens.css`
   (`--surface-*`, `--text-*`, `--border-*`, `--accent`, `--status-*`). No Tailwind, no CSS-in-JS.
   Match the existing neutral, low-contrast look.
4. **Terminal stability.** Don't introduce patterns that frequently unmount/remount `TerminalPane`.
   Always dispose xterm instances + addons on cleanup. PTYs must be killed on app exit (they are —
   `kill_all`) or ConPTY hosts (`OpenConsole.exe`) orphan and burn CPU.
5. **Version numbers are managed by `deploy.ps1` only.** Never hand-edit the `version` in
   `tauri.conf.json` / `package.json` / `Cargo.toml`. See [docs/VERSIONING.md](./docs/VERSIONING.md).
6. **Never ship unless the user explicitly asks.** Do NOT run `deploy.ps1` / publish a release on
   your own — multiple agents may be working at once, and a surprise release is hard to undo. Same
   for commit/push: only when asked. Otherwise work in dev mode (HMR); `main` is default, branch
   before committing if asked.
7. **Release notes are written at ship time, not per task.** Don't keep a running changelog while you
   work. When the user asks to ship, look at what changed since the last release
   (`git log <lastTag>..HEAD`) and write a few short user-facing bullets — pass them as the
   `deploy.ps1` notes; it records them in `docs/CHANGELOG.md` and the in-app "What's new".
8. **Code style:** human/casual, minimal comments (comment only tricky logic, keep it short and
   lowercase-casual). Match the surrounding code.

---

## Run / build / deploy (quick reference)

```bash
npm install
npm run tauri dev          # dev with HMR (Vite + Rust). This is how you work day-to-day.
npm run tauri build        # production build (Windows NSIS installer by default)
```

Releases are cut by a maintainer-local PowerShell script (`deploy.ps1`, not in this repo) that bumps
all three version files, builds + signs the Windows installer, publishes a GitHub release with a
`latest.json` manifest, and triggers the macOS CI build. It needs the project's signing key, so it's
maintainer-only.

**Verifying a change in dev:** TS changes hot-reload (run `npx tsc --noEmit` to typecheck). Rust
changes (`src-tauri/`) trigger a recompile + app relaunch — confirm with `cargo check` in
`src-tauri/` and that the rebuilt `target/debug/hyprspace-tauri.exe` is newer than your edit.

---

## Repo map

```
src/                         React frontend
  main.tsx                   entry (NO StrictMode), store hydration
  App.tsx                    shell layout
  App.css                    ordered @import index of styles/*.css (edit the per-area file, not this)
  styles/tokens.css          design tokens (theme variables)
  styles/<area>.css          per-area component CSS (rail, home, pane, loops, launcher, editor, …) —
                             split out of the old monolithic App.css so agents don't collide
  components/                UI: Titlebar, Rail (sidebar), PaneGrid, TerminalPane, HomePage,
                             ReviewDock, Settings, NewProjectDialog, CommandPalette,
                             LaunchWorkspace (multi-agent launcher), CodeEditor (dock editor),
                             LoopsPage + AutomationEditor + LoopRunView + LoopRunner (Automations),
                             StartupSettings, Logo, …
  stores/                    Zustand: workspace, ui, settings, settingsSync, git, activity, skills,
                             auth, updater, notifications, confirm, loops, launchPresets,
                             projectConfig, services, bridge (mobile)
  api/index.ts               typed bridge over Tauri invoke()/Channel — components import THIS,
                             never invoke() directly
  mobileBridge.ts            state mirror + action handler for the phone app (see mobile/)
  actions.ts                 shared actions (launch panes, worktrees, close) + provider cmd builders
  platform.ts                OS detection + platform-conditional bits (modifier keys, shells)
  themes.ts                  theme definitions applied over styles/tokens.css
  ai/                        autoNameSession.ts — titles a pane from the user's first prompt (Codex)
  lib/                       small helpers (projects.ts = where new projects go, time.ts) +
                             automations.ts (the Automations engine) + startup.ts (startup actions)

src-tauri/                   Rust backend
  src/lib.rs                 all #[tauri::command] registrations + app lifecycle (kill_all on exit)
  src/pty.rs                 PtyManager — ConPTY/portable-pty, byte coalescing
  src/agent.rs               AgentManager — one headless provider turn (used by the pane auto-namer)
  src/agenthook.rs           loopback listener feeding claude's hooks + status line into the app
                             (live agent state, usage meter, automation completion)
  src/bridge.rs              LAN WebSocket server the Android app talks to (off by default)
  src/devtools/              dev-cockpit commands, split into git.rs, worktree.rs, project.rs, fs.rs,
                             providers.rs, mcp.rs, skills.rs, usage.rs (per-provider usage read from
                             local CLI files, display-only) (+ mod.rs re-exports + shared helpers)
  src/persist.rs             crash-safe JSON state store (~/.hyprspace/v2)
  src/oauth.rs               loopback listener for the app's own Google/Supabase sign-in (PKCE)
  src/license.rs             Ed25519 license verification
  src/ai.rs                  ai_name_space (auto-name open spaces)
  tauri.conf.json            app config, version, updater endpoint + pubkey, capabilities
  capabilities/default.json  Tauri permission grants

mobile/                      the Android companion app — its own Expo + React Native app (see its README)
docs/                        documentation (start at docs/README.md)
website/                     the marketing site — its own Vite + React + Tailwind app (bun)
CONTRIBUTING.md              dev setup, style rules, PR flow (for outside contributors)
.github/workflows/release.yml  macOS CI build (merges darwin into the release manifest)
```

---

## Architecture in one screen

- **Spaces model.** A `workspace` is either a **project** (a folder, `kind !== "open"`) or an
  **open space** (`kind: "open"`, a scratch space whose panes can each be in a different folder).
  Each holds `sessions` (panes). State lives in `stores/workspace.ts`, persisted via `persist.rs`.
- **Panes = PTYs.** `TerminalPane` ↔ a `PtyManager` session. A pane runs a bare shell
  (`powershell`/`$SHELL`) and the launch command (e.g. `claude --permission-mode acceptEdits`) is
  **typed into the shell as keystrokes** — not passed as argv. Provider command strings come from
  `actions.ts` (`claudeCmd`/`geminiCmd`/`codexCmd`/`WSL_CMD`) and are constant (no user/LLM data
  interpolated into them).
- **Launcher.** `LaunchWorkspace` (opened from Home / palette / titlebar) fans out N agents in a
  folder at once: pick a folder → grid size → agent mix (quick-fill), then `addWorkspace` + N
  `addSession` calls so `PaneGrid` tiles them. Saved configs are `stores/launchPresets.ts`; agent
  panes get friendly names (`lib/names.ts`).
- **Editor.** `CodeEditor` (CodeMirror) lives in the Review dock's "Editor" tab; clicking a file in
  the Files tree opens it (`read_file`/`write_file` in `devtools/fs.rs`), with save / autosave.
- **Automations.** Scheduled / interval / manual agents. A `LoopDef` (persisted `"loops"`,
  `stores/loops.ts`) is driven by the engine in `lib/automations.ts`, which runs each fire in a
  **real claude pane** (ephemeral background tab, on the subscription — no API key, no headless
  path): it launches the constant `claudeCmd`, waits for the TUI's status line, **types the task in
  as keystrokes** (never onto a shell command line), and watches the pane's agent hooks for the
  turn to end. Every run has a wall-clock budget (defaulted) and hitting it **closes the pane** — it
  can't run forever. Optional worktree isolation + Review-changes. On the dedicated **Automations**
  page (rail); runs only while the app is open. See ARCHITECTURE.
- **IPC discipline.** Components call `src/api/index.ts` wrappers, never `invoke()` directly. Sync
  Tauri commands run on the UI thread, so anything filesystem-heavy is `async fn` + `spawn_blocking`.
- **Windows note.** `claude` is a `.cmd` shim, so it's spawned via `cmd /c claude …` so PATHEXT
  resolves it. Prompts go over stdin to avoid shell-escaping.

Full design details (session/cwd pinning, the Loops engine + hook backend, PTY coalescing):
**[docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md)**.

---

## Gotchas learned the hard way

- **`claude --resume <id>` is folder-scoped.** A session only resumes in the directory it was
  created in, so anything that resumes has to pin the `cwd` the session was created with. If a
  resume fails ("No conversation found"), drop the dead session id and start fresh.
- **Open spaces have no `cwd`** — code that pins/falls-back to cwd uses `??` (not `||`) so an empty
  string is preserved, not replaced.
- **CSP is currently `null`.** A strict CSP breaks Vite dev HMR, so it's a production-build task,
  not a dev change.
- **Persisted state names** are sanitized to a token in `persist.rs`, and large blobs are capped on
  save so the store can't grow unbounded.
- **An Automation can never run forever.** Every run gets a wall-clock budget (`stop.timeBudgetMin`,
  defaulted to 60 minutes when unset), and hitting it — like Stop — **closes the run's pane**, so
  the agent dies with the run. Don't add an infinite path, and don't let a stop path leave the
  pane's agent alive.

## Docs index
- [docs/README.md](./docs/README.md) — index of everything below
- [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) — how the tricky subsystems work
- [docs/VERSIONING.md](./docs/VERSIONING.md) — when to bump which digit
- [docs/BUILD-MAC.md](./docs/BUILD-MAC.md) — building the macOS app locally
