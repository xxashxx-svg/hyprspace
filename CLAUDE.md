# HyprSpace — project guide

> Read this first. It's the canonical guide for anyone (human or AI agent) working on this repo.
> Deep dives live in [`docs/`](./docs/README.md).

HyprSpace is a **multi-terminal AI workspace** — a Tauri 2 + React desktop app that tiles Claude
Code / Gemini / Codex / shell sessions across **projects** and **open spaces**, with per-pane
resume, drag-to-swap, a multi-agent **launcher** (fan out N agents in a folder at once), and a
command palette. Neutral, T3-Code-inspired dark UI.

- **Stack:** Tauri 2 (Rust) · React 19 + TypeScript + Vite · Zustand state · xterm.js (WebGL) ·
  `portable-pty` (Rust) · Supabase (auth only) · auto-update via Tauri updater + minisign.
- **Platforms:** Windows (primary, built locally) + macOS and Linux (both built in CI). Linux ships as
  an AppImage (self-updating) plus a `.deb` (no auto-update — Tauri's updater is AppImage-only).
  Platform-conditional UI wording lives in `src/platform.ts`; don't inline OS ternaries in components.
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
8. **Analytics stay boring and honest.** `src/lib/analytics.ts` sends ONE event (`app_opened`) with a
   random install id, version and OS — off in dev, off without `VITE_POSTHOG_KEY`, off at one click in
   Settings. This repo is public: if you add a property, the Settings copy and the file's header
   comment must change in the same commit. Never send prompts, terminal output, paths, project names,
   or anything joined to an account.
9. **Code style:** clear and conventional over casual. Comment the why, not the what. New UI code
   goes in a folder per area (`components/composer`, `components/dock`) with one stylesheet per
   area in `styles/`.
10. **Copy:** every string a user reads is plain English. No em dashes, no curly quotes, no
   filler. One idea per sentence. Say what happens, not how it feels.

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
  components/                UI: Titlebar, Rail (sidebar) + SessionRow, PaneGrid, TerminalPane,
                             HomePage, composer/ (ComposerPane, ModelPicker), dock/ (Dock, FilesPanel,
                             GitPanel), Settings, NewProjectDialog, CommandPalette, LaunchWorkspace
                             (multi-agent launcher), Menu (anchored dropdown), CodeEditor, Logo, …
  stores/                    Zustand: workspace, ui, settings, settingsSync, git, activity, skills,
                             agentStatus, usage, providers (installed CLIs), auth, updater,
                             notifications, confirm, launchPresets, bridge (mobile)
  api/index.ts               typed bridge over Tauri invoke()/Channel — components import THIS,
                             never invoke() directly
  mobileBridge.ts            state mirror + action handler for the phone app (see mobile/)
  actions.ts                 shared actions (launch panes, worktrees, close) + provider cmd builders
  platform.ts                OS detection + platform-conditional bits (modifier keys, shells)
  themes.ts                  theme definitions applied over styles/tokens.css
  ai/                        autoNameSession.ts — titles a pane from the user's first prompt (Codex)
  lib/                       models.ts (agent catalog: models, efforts, flags), composer.ts (type a
                             prompt into a pane once its CLI is up), agentHeuristics.ts (row state
                             for CLIs without hooks), grid.ts (layouts + resizable boundaries),
                             brand.ts (provider marks), projects.ts, time.ts, branches.ts

src-tauri/                   Rust backend
  src/lib.rs                 all #[tauri::command] registrations + app lifecycle (kill_all on exit)
  src/pty.rs                 PtyManager — ConPTY/portable-pty, byte coalescing
  src/agent.rs               AgentManager — one headless provider turn (used by the pane auto-namer)
  src/agenthook.rs           loopback listener feeding claude's hooks + status line into the app
                             (live agent state, usage meter)
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
- **Composer.** `components/composer/ComposerPane` is where a session starts: pick the agent, model
  and effort (`ModelPicker`, catalog in `lib/models.ts`, installed CLIs from `stores/providers.ts`),
  type a task, press Enter. "New session" (`actions.newSession`) adds a **draft** session
  (`Session.draft`) that renders as a composer pane in the grid; submitting calls `startDraft`, which
  turns it into a normal pane under the same id, and `lib/composer.ts` **types the prompt in as
  keystrokes** once the CLI is up (claude: its status line; other CLIs: a fixed delay). An empty
  space shows a composer too. Under the box: the CLI's saved conversations for the folder
  (`agent_sessions` reads claude's transcripts and codex's rollouts; `actions.resumeCmd` reopens
  one), and a clone card when the text starts with a repository link (`git_clone`). Terminal path
  only, no SDK, no token.
- **Model / effort flags.** `actions.commandFor(provider, choice)` builds the launch command; the
  per-agent defaults live in settings (`agentModel`, `agentEffort`) and the composer edits them.
  Claude takes `--effort low|medium|high|xhigh|max`, Codex `-c model_reasoning_effort=...`. Effort
  levels are per model: Codex's list (and each model's levels, up to `ultra`) is read from its own
  `~/.codex/models_cache.json` by `stores/providers.ts`; the static catalog is the fallback.
- **Sidebar.** One resizable column (`Rail`): search, then every space as a section that folds open
  to its threads (`SessionRow`), the active space open by default with its working tree's file
  count and line deltas. Rows drag to reorder; dropping on another space moves the pane there.
  A space can be archived (`Workspace.archived`): it parks under an "Archived" group at the bottom
  with its panes still running. Row state comes from
  claude's hooks (`stores/agentStatus`) or, for CLIs without hooks, from the terminal output
  (`lib/agentHeuristics`: recent output = working, a question in the last lines = waiting).
- **Right dock.** `components/dock/Dock` (Ctrl+Shift+G): Files (`FilesPanel`, a lazy tree with git
  decorations) and Git (`GitPanel`, tick files to stage, summary + description, commit to the branch,
  push when ahead). It follows the focused pane's folder. Resizable from its left edge.
- **Pane grid.** Layout presets in `lib/grid.ts`; the boundaries between tracks that no pane spans
  are draggable (`resizableBoundaries`), and dragged weights persist per layout in
  `Workspace.tracks`. The pane header is a grip, the agent mark, the name, and a close button;
  double-click it to maximize.
- **Editor.** `CodeEditor` (CodeMirror) opens as a pane tab when you ctrl+click a file path in a
  terminal, or a file in the dock's Files tab. A changed file in the Git tab opens its diff as a
  pane the same way (`DiffViewer`, `Session.diff`).
- **IPC discipline.** Components call `src/api/index.ts` wrappers, never `invoke()` directly. Sync
  Tauri commands run on the UI thread, so anything filesystem-heavy is `async fn` + `spawn_blocking`.
- **Windows note.** `claude` is a `.cmd` shim, so it's spawned via `cmd /c claude …` so PATHEXT
  resolves it. Prompts go over stdin to avoid shell-escaping.

Full design details (session/cwd pinning, the hook backend, PTY coalescing):
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

## Docs index
- [docs/README.md](./docs/README.md) — index of everything below
- [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) — how the tricky subsystems work
- [docs/VERSIONING.md](./docs/VERSIONING.md) — when to bump which digit
- [docs/BUILD-MAC.md](./docs/BUILD-MAC.md) — building the macOS app locally
- [docs/BUILD-LINUX.md](./docs/BUILD-LINUX.md) — building the Linux AppImage/`.deb`, WebKitGTK notes
