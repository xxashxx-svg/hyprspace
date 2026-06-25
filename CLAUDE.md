# HyprSpace — project guide

> Read this first. It's the canonical guide for anyone (human or AI agent) working on this repo.
> Deep dives live in [`docs/`](./docs/README.md).

HyprSpace is a **multi-terminal AI workspace** — a Tauri 2 + React desktop app that tiles Claude
Code / Gemini / Codex / shell sessions across **projects** and **open spaces**, with per-pane
resume, drag-to-swap, a command palette, a git review dock, and a home-screen AI chat that can
operate the app. Neutral, T3-Code-inspired dark UI.

- **Stack:** Tauri 2 (Rust) · React 19 + TypeScript + Vite · Zustand state · xterm.js (WebGL) ·
  `portable-pty` (Rust) · Supabase (auth only) · auto-update via Tauri updater + minisign.
- **Platforms:** Windows (primary, built locally) + macOS (built in CI). Linux mostly works but isn't shipped.

---

## ⚠️ Critical constraints — do not violate

1. **Subscription compliance (most important).** The app runs Claude on the user's **subscription**
   by spawning their already-logged-in `claude` CLI — nothing else. **NEVER** build a custom
   claude.ai OAuth flow, **NEVER** read/store/forward the subscription OAuth token to call the
   Anthropic API yourself, **NEVER** feed subscription tokens to an SDK. Reading credential files
   for **display-only** fields (email / plan) is fine; using a token for auth is not. This is the
   same sanctioned path T3 Code uses. The home chat (`chat.rs`) and the terminal panes both just
   spawn the CLI.
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
6. **Don't commit/push/deploy unless explicitly asked.** Work in dev mode (HMR). The default branch
   is `main`; branch before committing if asked to commit.
7. **Code style:** human/casual, minimal comments (comment only tricky logic, keep it short and
   lowercase-casual). Match the surrounding code.

---

## Run / build / deploy (quick reference)

```bash
npm install
npm run tauri dev          # dev with HMR (Vite + Rust). This is how you work day-to-day.
npm run tauri build        # production build (Windows NSIS installer by default)
```

```powershell
.\deploy.ps1 patch "what changed"   # bump + build + sign + publish a GitHub release & update feed
```

`deploy.ps1 <none|patch|minor|major>` bumps all three version files, builds + signs the Windows
installer, publishes to the public `hyprspace-releases` repo with a `latest.json` manifest, and
triggers the macOS CI build. Full runbook (incl. doing it without Claude): [docs/DEPLOY.md](./docs/DEPLOY.md).

**Verifying a change in dev:** TS changes hot-reload (run `npx tsc --noEmit` to typecheck). Rust
changes (`src-tauri/`) trigger a recompile + app relaunch — confirm with `cargo check` in
`src-tauri/` and that the rebuilt `target/debug/hyprspace-tauri.exe` is newer than your edit.

---

## Repo map

```
src/                         React frontend
  main.tsx                   entry (NO StrictMode), store hydration
  App.tsx / App.css          shell layout + all component CSS
  styles/tokens.css          design tokens (theme variables)
  components/                UI: Titlebar, Rail (sidebar), PaneGrid, TerminalPane, HomePage,
                             ChatPanel, ReviewDock, Settings, NewProjectDialog, CommandPalette,
                             LoopsPage + LoopsManager + LoopRunner (Loops), StartupSettings, …
  stores/                    Zustand: workspace, ui, settings, settingsSync, chat, orchestrator,
                             git, activity, skills, auth, updater, notifications, confirm, loops,
                             projectConfig, services
  api/index.ts               typed bridge over Tauri invoke()/Channel — components import THIS,
                             never invoke() directly
  actions.ts                 shared actions (launch panes, worktrees, close) + provider cmd builders
  lib/                       small helpers (projects.ts = where new projects go, time.ts) +
                             loops.ts (the Loops engine)

src-tauri/                   Rust backend
  src/lib.rs                 all #[tauri::command] registrations + app lifecycle (kill_all on exit)
  src/pty.rs                 PtyManager — ConPTY/portable-pty, byte coalescing
  src/chat.rs                ChatManager — the persistent home-chat claude process (stream-json)
  src/agent.rs               AgentManager — runs ONE provider turn (claude -p …) for the Loops engine
  src/services.rs            ServiceManager — per-folder startup services (background processes)
  src/devtools.rs            git ops, provider_status, skills, mcp, worktrees, create_project_dir, reveal_path
  src/persist.rs             crash-safe JSON state store (~/.hyprspace/v2)
  src/oauth.rs               loopback listener for the app's own Google/Supabase sign-in (PKCE)
  src/license.rs             Ed25519 license verification
  src/ai.rs                  ai_name_space (auto-name open spaces)
  tauri.conf.json            app config, version, updater endpoint + pubkey, capabilities
  capabilities/default.json  Tauri permission grants

docs/                        documentation (start at docs/README.md)
deploy.ps1                   Windows release script
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
- **Home chat = a persistent `claude` stream-json process.** `ChatPanel` → `stores/chat.ts` →
  `chat.rs`. One long-lived process per thread (`--input-format stream-json`), fed user turns over
  stdin, streaming events back. Runs on the subscription (spawns the CLI). See ARCHITECTURE.
- **Orchestrator.** The chat model can operate the app by emitting ```hyprspace fenced JSON blocks
  (create_project / new_open_space / spawn_agents / switch_space) which `stores/orchestrator.ts`
  parses and executes against the stores. It's in-process text directives — **not** an MCP bridge.
- **Loops.** Scheduled / interval / until-done / manual agents. A `LoopDef` (persisted `"loops"`,
  `stores/loops.ts`) is driven by the engine in `lib/loops.ts`, which runs each iteration through the
  headless agent runner (`agent.rs`). Pluggable backends: **Claude** (on a user Anthropic API key from
  the OS keychain — never the subscription), **Codex**, **Gemini**. Every loop **must** declare a stop
  limit (mandatory max-iterations; optional sentinel / `untilCheck` command / time budget / no-progress
  auto-stop) — it can't run forever. Optional worktree isolation + Review-changes. On the dedicated
  **Loops** page (rail); runs only while the app is open. See ARCHITECTURE.
- **IPC discipline.** Components call `src/api/index.ts` wrappers, never `invoke()` directly. Sync
  Tauri commands run on the UI thread, so anything filesystem-heavy is `async fn` + `spawn_blocking`.
- **Windows note.** `claude` is a `.cmd` shim, so it's spawned via `cmd /c claude …` so PATHEXT
  resolves it. Prompts go over stdin to avoid shell-escaping.

Full design details (chat protocol, session/cwd pinning, watchdog, orchestrator format, PTY
coalescing): **[docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md)**.

---

## Gotchas learned the hard way

- **`claude --resume <id>` is folder-scoped.** A session only resumes in the directory it was
  created in. The chat pins a `cwd` per thread for this reason; if a resume fails ("No conversation
  found"), the thread forgets the dead session and starts fresh on the next message.
- **The chat parser must never render blank.** It falls back to the complete `assistant` event when
  streaming yields only a thinking block, and an inactivity watchdog unsticks a wedged turn.
- **Open spaces have no `cwd`** — code that pins/falls-back to cwd uses `??` (not `||`) so an empty
  string is preserved, not replaced.
- **CSP is currently `null`** (see [docs/audit/security.md](./docs/audit/security.md) S1). A strict
  CSP breaks Vite dev HMR, so it's a production-build task, not a dev change.
- **Persisted state names** are sanitized to a token in `persist.rs`; the chat blob is capped
  (30 threads, 200 msgs/thread, tool results truncated) on save.
- **A Loop can never run forever.** `LoopStop.maxIterations` is mandatory by construction, and the
  engine also auto-stops on no-progress (3 consecutive unchanged/empty iterations → `crashloop`),
  an optional sentinel token in the output, and an optional time budget. Don't add an infinite path.

## Docs index
- [docs/README.md](./docs/README.md) — index of everything below
- [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) — how the tricky subsystems work
- [docs/DEPLOY.md](./docs/DEPLOY.md) — release runbook (no Claude required)
- [docs/VERSIONING.md](./docs/VERSIONING.md) — when to bump which digit
- [docs/ENTITLEMENT.md](./docs/ENTITLEMENT.md) — subscription gating: how to go free → paid
- [docs/audit/](./docs/audit/README.md) — security + bug audit and its status
- [BUILD-MAC.md](./BUILD-MAC.md) — building the macOS app locally
