# Architecture

How the non-obvious subsystems work. For the high-level map + constraints, see
[../CLAUDE.md](../CLAUDE.md).

## Topology

A Tauri app = a Rust **main process** + a **webview** (the React app). They talk over Tauri IPC:
- React → Rust: `invoke("command", args)`. **All commands are wrapped in `src/api/index.ts`** —
  components import those typed wrappers, never `invoke()` directly.
- Rust → React: a `Channel<T>` (streaming, used for PTY bytes and chat events) or events.
- Sync `#[tauri::command]` runs on the UI thread → anything slow/filesystem-heavy is `async fn` +
  `tauri::async_runtime::spawn_blocking` so the window never freezes.

The webview only ever loads **local bundled assets** — no remote page is loaded into an
IPC-privileged window. The realistic threat to the IPC surface is a renderer XSS, which is why CSP
matters (see [audit/security.md](./audit/security.md) S1). Markdown is rendered without raw HTML.

## Spaces & sessions (`stores/workspace.ts`)

- A **workspace** is a **project** (has a `cwd` folder) or an **open space** (`kind: "open"`, a
  scratch space; panes can each target a different folder, so the space itself may have no `cwd`).
- A workspace holds `sessions` (panes). Each session has an id, a launch `command`, a `cwd`, a
  `provider` (claude/gemini/codex/terminal/wsl), and runtime flags (started, etc.).
- The store persists to disk via `persist.rs` (debounced in `App.tsx`).
- Because open spaces can have an empty `cwd`, cwd fallbacks use `??` (preserve `""`) not `||`.

## PTY subsystem (`pty.rs` ↔ `TerminalPane.tsx`)

- `PtyManager` owns `Mutex<HashMap<id, Session>>`. `create` spawns a shell via `portable-pty`
  (ConPTY on Windows) with `args: []` — i.e. a bare `powershell`/`$SHELL`. The pane's launch command
  is **typed into the shell as keystrokes** (`writePty(... toRun + "\r")`), never passed as argv.
  So no user/LLM string becomes a process argument.
- **Byte coalescing:** stdout is read on a thread and coalesced (small time/size window) before
  being sent over the `Channel` so fast output doesn't flood the UI but interactive echo stays snappy.
- **Lifecycle:** a wait thread emits an `Exit` control when the child dies. `kill_all()` runs on app
  exit (`lib.rs` RunEvent) and drops every master PTY → `ClosePseudoConsole` → ends the ConPTY host.
  This is essential: orphaned `OpenConsole.exe` hosts busy-spin at high CPU. Locks use a
  poison-tolerant helper (`unwrap_or_else(|e| e.into_inner())`).

## Persistent home chat (`chat.rs` ↔ `stores/chat.ts` ↔ `ChatPanel.tsx`)

The home-screen chat runs Claude **on the subscription** by spawning the user's `claude` CLI in
**stream-json** mode — no API key, no SDK, no token handling.

### Transport (`chat.rs`)
One **long-lived process per thread**:
```
claude -p --input-format stream-json --output-format stream-json --include-partial-messages --verbose
       [--model X] [--resume <sid>] [--permission-mode Y]
```
- `start(id, cwd, args, ch)` — spawns it (via `cmd /c claude …` on Windows so the `.cmd` shim
  resolves), keeps **stdin open**, streams every stdout line to the `Channel` for the process's
  whole life. On stdout EOF (process gone) it emits an `{"type":"exit"}` sentinel.
- `turn(id, message)` — writes one user-message JSON envelope (a single line) to the live stdin:
  `{"type":"user","message":{"role":"user","content":[{"type":"text","text":"…"}]}}`.
- A **turn is complete on the `result` event** — the process stays alive for the next turn (no
  respawn, no `--resume` between turns). `--resume` is only used to recover an existing conversation
  when a fresh process is started (process died, app restarted, thread switched).
- `stop`/`kill_all` reap the tree (`taskkill /T /F` on Windows after closing stdin).

### Driver (`stores/chat.ts`)
- A module-level `live` session owns the current process. Each `chatStart` binds its event handler
  to a **specific session object** (`routeLine(sess, …)`), and the handler ignores lines when
  `live !== sess` — so events from a superseded or dead process can't clobber the new one.
- Per-turn parse state (gotStream/skipBlock/startedAt/pending text + throttle) lives on the session.
  Token deltas are throttled (~55ms) so markdown doesn't re-render every token.
- **Never render blank:** if streaming produced only a thinking block, the handler falls back to the
  complete `assistant` event; if a turn ends with nothing, a `(no response)` placeholder is shown.
- **Watchdog:** an inactivity timer (reset on every event, ~6 min) unsticks a wedged turn —
  surfaces a timeout note and clears `busy`. Generous so long agentic turns aren't interrupted.
- **cwd pinning:** the thread records the folder it first ran in (`thread.cwd`) and every later
  turn/resume uses it — because `claude --resume` is **folder-scoped** (a session only resumes in
  the directory it was created in). If a resume fails ("No conversation found"), the thread drops
  the dead `sessionId` so the next message starts fresh.
- **Persistence is bounded:** newest 30 threads, last 200 messages/thread, tool `result` strings
  truncated. `load()` defensively normalizes the blob so a corrupt/old shape can't white-screen.

### UI (`ChatPanel.tsx`)
Collapsible: a launcher (closed) → composer (open) → composer + history (expanded). Auto-expands on
send. The "continue in a terminal pane" button resumes the thread's session **in the thread's
folder** (and prefers a project already at that folder) so `--resume` works.

## Orchestrator (`stores/orchestrator.ts`)

The chat model can operate the app. It's an **in-process operator-command protocol** — NOT an MCP
bridge. A system preamble (`ORCHESTRATOR_PREAMBLE`) is injected into the first turn telling the
model it can emit fenced blocks:
````
```hyprspace
{"action":"create_project","name":"my-app"}
```
````
On turn completion, `runOperatorText` strips those blocks from the visible text and `executeCommand`
runs them against the stores. Actions: `create_project`, `new_open_space`, `spawn_agents`
(provider whitelisted, count clamped 1–8), `switch_space`. Results render as action chips.

**Safety:** `create_project` is confined to the projects base folder — `slug()` strips every
non-`[A-Za-z0-9_-]` char (so `..`, `/`, `\`, `:` collapse), and a `startsWith(base)` check is the
backstop. The model **cannot** supply an arbitrary path (the `path` field was removed). Spawned
panes use the constant provider command builders, so no model text reaches a process argument.

Where new projects go: `src/lib/projects.ts` → `projectsBaseDir()` (the Settings → Workspace
"Projects folder", default `~/Documents/HyprSpace`) + `joinPath`. Both the chat and the New Project
dialog use it so they agree.

## Provider command builders (`actions.ts`)

`claudeCmd(mode)`, `geminiCmd(yolo)`, `codexCmd(mode)`, `WSL_CMD` produce **constant** command
strings from the user's Settings → Providers preferences (no interpolation of dynamic/LLM data).
`launchInActive` adds a session to the active space — and for open spaces pops a folder picker so
each pane can target a different folder. This is the single launch path the top **New** menu uses.

## Persistence (`persist.rs`)

A single-writer, crash-safe JSON store at `~/.hyprspace/v2/<name>.json`: temp file → fsync →
atomic rename, under one (poison-tolerant) lock. `load` distinguishes "file absent" (Ok(None)) from
"IO error" (Err) so a transient error never looks like a first run and clobbers data. The `name` is
sanitized to a safe token so it can't traverse out of the dir. The TS stores (`workspace`,
`settings`, `chat`, …) serialize their state into this.

## Auth (`oauth.rs` + `lib/supabase.ts` + `stores/auth.ts`)

The app's **own** sign-in (Google via Supabase) — entirely separate from claude.ai. It uses a
loopback listener (`127.0.0.1:8765`) for the OAuth redirect and PKCE (Supabase validates the
`code_verifier`). This is **not** the Claude subscription auth — that's handled entirely by the
`claude` CLI we spawn. Credential files are only ever read for display-only fields.

## Provider status (`devtools.rs::provider_status`)

For Settings → Providers: runs `<cli> --version` (args passed separately, never a shell string) and
reads `~/.claude.json` / `.credentials.json` / Codex `auth.json` for **display-only** account/plan
fields (a JWT is base64-decoded **without verification**, purely to show email/plan — no trust
decision, never forwarded).
