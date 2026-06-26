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

## Startup services (`services.rs` ↔ `StartupSettings.tsx`)

Per-folder startup tasks that run as **background processes** (no PTY, no pane) so a dev server /
watcher can run while its output stays viewable in the Services panel — the "Run in background"
option. Configured in Settings → Startup; the per-folder list + env live in `stores/projectConfig.ts`.

- `ServiceManager` (`services.rs`) owns a `Mutex<HashMap<id, Child>>`. `start(id, cwd, command, env, ch)`
  spawns the command through a shell — `powershell -NoProfile -NonInteractive -Command "<command>"`
  on Windows, `sh -c "<command>"` elsewhere — with `stdin` nulled, streaming every stdout+stderr line
  to a `Channel`. On stdout EOF it sends the exit sentinel `\u{0}__service_exit__` so the UI marks the
  service stopped.
- Like the agent runner it clears `NoDefaultCurrentDirectoryInExePath` so a script behaves like a
  double-click (an `.exe` next to a `.bat` still resolves).
- Commands: `service_start` / `service_stop` (`api/index.ts` → `serviceStart`/`serviceStop`). Reaped
  by `kill_all` on app exit (`taskkill /T /F` on Windows), same as PTYs and the chat process.

## Loops (`stores/loops.ts` + `lib/loops.ts` + `agent.rs`)

HyprSpace's scheduled / looping agents. A **`LoopDef`** is a saved definition (persisted as `"loops"`,
a map keyed by id); its live `LoopRun` (status / iteration / logs / stale counter / worktree path) is
**in-memory only** — loops run only while the app is open. UI: a dedicated **Loops & Automations**
page reached from the rail (`LoopsPage.tsx` wrapping `LoopsManager.tsx`), plus command-palette entries
and a titlebar/rail badge while any run. The page leads with one-click starter templates
(`lib/loopTemplates.ts`).

### Backends (pluggable — and the subscription is never used)
Each loop picks a `provider`. **Loops deliberately do not use the Claude subscription** — that path is
reserved for the panes + home chat. Instead:
- **Claude** runs on a user-provided **Anthropic API key** stored in the OS keychain (Windows
  Credential Manager / macOS Keychain via `secret_set`/`secret_has`/`secret_clear`). The key is read
  in Rust at spawn time and injected as `ANTHROPIC_API_KEY` — it never crosses into the webview. A
  Claude loop **refuses to start** without a key (otherwise `claude -p` would fall back to the
  subscription, which it must not).
- **Codex** uses your `codex login` auth; **Gemini** uses its own CLI login. Neither needs a key.

### Engine (`lib/loops.ts`)
`startLoop`/`stopLoop`/`pauseLoop` drive a per-loop controller held in a module `Map` (so each loop
has one in-flight chain; `isLoopActive(id)` checks it). Each iteration runs **one provider turn
headless**: `buildArgs(def, cont)` produces the argv and `runIteration` calls `agentStart` (with the
keychain `secrets` map), piping the prompt over stdin and resolving when the turn's exit sentinel
arrives. Per backend:
- **claude** — `claude -p --output-format stream-json --verbose [--model X] --permission-mode <mode>`;
  the stream-json events are parsed into readable log lines (`→ Read/Edit/Bash …`, the agent's text,
  and a final `✓ done · $cost`).
- **codex** — `codex exec [-m X] -s <sandbox> --skip-git-repo-check`.
- **gemini** — `gemini [-m X] -p "" --approval-mode <mode> --skip-trust`.

`permissionMode` (plan / acceptEdits / bypass / default) maps onto each CLI's own vocabulary
(`--permission-mode bypassPermissions`, codex `-s read-only|workspace-write` or
`--dangerously-bypass-approvals-and-sandbox`, gemini `--approval-mode plan|auto_edit|yolo`). Modes:
- **until-done** — re-prompt back-to-back with a ~1.2s breather between iterations.
- **interval** — every `intervalSec` seconds (min 5).
- **cron** ("Schedule") — `nextFire(schedule)` from `everyMin` or a daily `HH:MM` (raw 5-field cron is
  a typed-but-not-yet-implemented field); defaults hourly.
- **manual** — runs exactly once.

**Session.** `session: "continue"` keeps context across iterations — the first run seeds the session,
the rest add the backend's continue/resume flag (`claude --continue` / `codex exec resume --last -` /
`gemini --resume latest`). `"fresh"` starts each iteration clean.

**Worktree isolation.** With `worktree` on and the folder a git repo, the run happens in a throwaway
worktree (branch `hs/loop-<id>`, idempotent so re-runs reuse it) so an autonomous agent never touches
the working tree until you review the diff; the card exposes a **Review changes** button. Falls back
to running in place when the folder isn't a repo. (`run: "pane"` is in the data model but not yet wired.)

### Stop guards (a loop can never run forever)
Enforced in the tick loop, in this order:
- **`maxIterations`** — mandatory hard cap → status `done`.
- **`timeBudgetMin`** — optional wall-clock cap → status `stopped`.
- **no-progress** — if `noProgress` is on and 3 iterations in a row produce identical (output hashed
  with the multiply-by-31 mod 2147483647 family) or empty output, it auto-stops → status `crashloop`.
- **sentinel** — if the output contains the `sentinel` token (e.g. `LOOP_DONE`) → status `done`.
- **untilCheck** — an optional shell command run after each iteration (`run_check` in `devtools.rs`,
  in the loop's folder/worktree); exit `0` means the goal is met (e.g. `npm test` passes) → `done`.

(`tokenBudget` exists on `LoopStop` but isn't wired in yet — it needs token counts from the JSON output.)

### Agent runner (`agent.rs`)
`AgentManager` runs **one** provider turn per call — the headless analog of `chat.rs`. `start(id, cwd,
args, env, secrets, prompt, ch)` spawns the full argv (via `cmd /c …` on Windows so the `.cmd` shim
resolves), writes the prompt to stdin then closes it (so a long/multiline prompt never has to survive
shell quoting), and streams stdout+stderr as lines. `secrets` maps an env-var name → a keychain secret
name (e.g. `ANTHROPIC_API_KEY` ← `"anthropic"`); each is read from the OS keychain **in Rust** and set
on the child env, so the value never enters JS. On stdout EOF it emits `\u{0}__agent_exit__` so the
runner advances the loop. Commands: `agent_start` / `agent_stop` (`api/index.ts` → `agentStart`/
`agentStop`); the run id is `loop:<defId>`. Reaped by `kill_all` on exit like the other managers.

`LoopRunner.tsx` mounts once, hydrates saved loops, and auto-starts the **enabled** `cron`/`interval`
loops; `until-done` and `manual` loops are started by hand from the UI.

## Multi-agent launcher (`LaunchWorkspace.tsx` + `stores/launchPresets.ts`)

Fan out many agents at once: pick a working folder, a grid size (1–12 terminals), and an agent mix
(per-provider counts with quick-fill — All Claude / One of each / Split evenly). Launch creates a
project (`addWorkspace`) then loops `addSession` for each agent command, so `PaneGrid` tiles them.
Agent panes get a short friendly name (`lib/names.ts`) so identical agents are tellable apart. A
config (folder + grid + mix) can be saved as a **preset** (`stores/launchPresets.ts`, persisted
`"launchPresets"`) and relaunched in one click. Opened from Home, the command palette, or the
titlebar **New** menu.

## Integrated editor (`CodeEditor.tsx` + `devtools/fs.rs`)

A CodeMirror 6 editor in the Review dock's **Editor** tab. Clicking a file in the Files tree (or its
context menu) calls `openInEditor` (`stores/ui.ts`), which reads the file via `read_file`
(`devtools/fs.rs`, capped at 2 MB, rejects binary) and shows it with syntax highlighting; **Ctrl/⌘+S**
or the autosave toggle writes it back via `write_file`. Themed to the app tokens with one-dark colors.

## Code structure & animation notes

- **CSS is split per area.** `src/App.css` is just an ordered `@import` index of `src/styles/*.css`
  (one file per area: rail, home, pane, loops, launcher, editor, …). Edit the area file, not the
  index; order is preserved so the cascade is identical to the old single file.
- **`devtools` is a folder module** (`git` / `worktree` / `project` / `fs` / `providers` / `mcp` /
  `skills`), re-exported by `mod.rs` so `devtools::*` paths in `lib.rs` are unchanged. Shared helpers
  (`git`, `home_dir`, `read_json`) live in `mod.rs`.
- **Smooth UI** uses `@formkit/auto-animate` (rail lists + expand/collapse, the file tree, the Loops
  list, launcher presets) plus a `.no-transitions` guard toggled in `applyTheme` so a theme switch
  snaps colors instead of animating every element. `prefers-reduced-motion` is respected app-wide.
- **Dev-state isolation.** `persist.rs` honors a `HYPRSPACE_STATE_DIR` env override (unset in release
  builds) so a dev instance can run on a scratch state dir without touching the user's `~/.hyprspace/v2`.

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

## Provider status (`devtools/providers.rs::provider_status`)

For Settings → Providers: runs `<cli> --version` (args passed separately, never a shell string) and
reads `~/.claude.json` / `.credentials.json` / Codex `auth.json` for **display-only** account/plan
fields (a JWT is base64-decoded **without verification**, purely to show email/plan — no trust
decision, never forwarded).
