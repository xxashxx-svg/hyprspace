# Architecture

How the non-obvious subsystems work. For the high-level map + constraints, see
[../CLAUDE.md](../CLAUDE.md).

## Topology

A Tauri app = a Rust **main process** + a **webview** (the React app). They talk over Tauri IPC:
- React → Rust: `invoke("command", args)`. **All commands are wrapped in `src/api/index.ts`** —
  components import those typed wrappers, never `invoke()` directly.
- Rust → React: a `Channel<T>` (streaming, used for PTY bytes, agent/service output) or events.
- Sync `#[tauri::command]` runs on the UI thread → anything slow/filesystem-heavy is `async fn` +
  `tauri::async_runtime::spawn_blocking` so the window never freezes.

The webview only ever loads **local bundled assets** — no remote page is loaded into an
IPC-privileged window. The realistic threat to the IPC surface is a renderer XSS, which is why CSP
matters.

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

## Where new projects go (`lib/projects.ts`)

`projectsBaseDir()` resolves the Settings → Workspace "Projects folder" (default
`~/Documents/HyprSpace`) and `joinPath` builds the folder under it. Anything that creates a project —
today the New Project dialog — goes through it so they all agree on the location.

## Startup actions (`lib/startup.ts` ↔ `StartupSettings.tsx` + `StartupRunner.tsx`)

Per-folder startup tasks (dev server, db, watchers) configured in Settings → Startup; the per-folder
list + env live in `stores/projectConfig.ts`. Each task launches as a normal **terminal pane**
(`addSession` with the task's command) — there is no separate background-process runtime. Config is
keyed by folder (`folderKey`), so two projects at the same folder share one config, and launches are
deduped by command across every workspace at that folder so the same server never starts twice.
`maybeAutostart` fires the folder's `runOnOpen` actions the first time it's opened each session;
actions flagged `runOnWorktree` also fire when a worktree is created for the folder.

## Automations (`stores/loops.ts` + `lib/automations.ts`)

HyprSpace's scheduled agents. A **`LoopDef`** is a saved definition (persisted as `"loops"`, a map
keyed by id); its live `LoopRun` (status / logs / worktree path / host pane) is **in-memory only** —
automations run only while the app is open. Finished runs land in a persisted per-automation history
(`"loop-history"`). UI: the **Automations** page reached from the rail (`LoopsPage.tsx`, with
`AutomationEditor.tsx` as the inline editor and `LoopRunView.tsx` as the run view), plus
command-palette entries and a titlebar badge while any run.

### The engine runs in a real pane (on the subscription)
When an automation fires, the engine (`lib/automations.ts`):
1. optionally cuts a **worktree** (`worktreeCreate`, branch `hs/auto-<name>`, idempotent) so the
   agent can't touch the working tree — the run view exposes **Review changes**; a folder that isn't
   a git repo just runs in place;
2. resolves the folder's workspace (creating one **without activating it** — a scheduled fire must
   never switch the space you're looking at) and launches a normal claude pane as a background tab
   (`addTab`/`addSession` with `focus: false, ephemeral: true`). The launch command is the constant
   `claudeCmd(permissionMode)` — same path as any pane, on the logged-in CLI / subscription;
3. waits for the pane's claude TUI to come up — its **status line** reporting for that pane
   (`useUsage.byPane`) is the readiness signal — then **types the task into the TUI** as keystrokes.
   The prompt never rides a shell command line, so there is nothing to quote and no shell that could
   misparse it. If the TUI never reports (CLI missing), the run errors out instead of typing at a
   bare shell;
4. watches the pane's **agent hooks** (`useAgentStatus`): `Stop` after a `working` state = the run
   is `done`.

**Ephemeral panes.** An automation's pane is marked `ephemeral: true`: `PaneGrid` mounts it even in
a space you haven't opened this session (without spawning the space's other saved panes), and
`App.tsx`'s save filter drops it from the persisted layout — a saved automation pane would relaunch
its agent on the next app start.

### Modes & scheduling
- **manual** — runs once when you hit Run.
- **interval** — every `intervalSec`; **cron** — `nextFire(schedule)` from `everyMin`, a daily
  `HH:MM`, or a raw 5-field cron (`lib/cron.ts`).
- Scheduled automations **re-arm after each run** (`finish()` arms the next fire instead of tearing
  the controller down), and ones with `enabled` set are armed on app start by `LoopRunner.tsx`.
  Manual automations never auto-run.

### The stop guard (an automation can never run forever)
Every run has a **wall-clock budget** (`stop.timeBudgetMin`, defaulted to 60 when unset). Hitting it
calls `finish("error")`, which **closes the run's pane** — the agent dies with the run rather than
grinding on unwatched. `stopLoop` closes the pane the same way; the only pane that outlives its run
is a successfully finished one-shot, kept so you can read what the agent did. There is no headless
path, no API key, and no keychain involvement — the retired headless engine (per-iteration
`claude -p` on an `ANTHROPIC_API_KEY`) was deleted with `lib/loops.ts`/`loophook.rs`, and old defs
(other providers, `maxIterations`-era stop guards) are migrated onto this engine on load.

### Agent runner (`agent.rs`)
`AgentManager` runs **one** provider turn per call, headless — no PTY, no pane. Today its only
caller is the pane auto-namer (`ai/autoNameSession.ts`, one short `codex exec` per unnamed pane).
`start(id, cwd, args, env, secrets, prompt, ch)` spawns the argv (via `cmd /c …` on Windows so the
`.cmd` shim resolves), writes the prompt to stdin then closes it, and streams stdout+stderr as
lines; on EOF it emits `\u{0}__agent_exit__`. `secrets` maps an env-var name → an OS-keychain secret
name, read **in Rust** and set on the child env so the value never enters JS (unused by current
callers, which pass `{}`). Reaped by `kill_all` on exit like the PTYs.

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

## Mobile bridge (`bridge.rs` + `mobileBridge.ts` ↔ [`mobile/`](../mobile/README.md))

How the Android companion app sees your desktop. Off by default; **Settings → Mobile** turns it on and
shows the pairing QR.

**Transport.** `bridge.rs` is a hand-rolled WebSocket server on the LAN (6768 by default, walking up
to 8 ports if that's taken). Hand-rolled because the framing we need is ~100 lines and it keeps a
network-facing dependency tree out of an app that otherwise has none; `sha1` for the handshake digest
is the only addition. A connection must send `hello` with the pairing token within 8s or it's dropped,
and `PROTOCOL` must match on both sides — a version mismatch is reported rather than half-working.
The token is minted and persisted by the frontend (`stores/bridge.ts`, `crypto.getRandomValues`);
Rust only ever compares against it, in constant time.

**State is pushed, never introspected.** Rust knows nothing about spaces or panes. `mobileBridge.ts`
subscribes to the workspace / agent-status / usage / automations stores, debounces 250 ms, and calls
`bridge_publish` with a snapshot whenever it actually changed. The bridge stores the last one verbatim
and fans it out, so a phone's lists move the moment the desktop's do — and a phone connecting later
gets the current picture immediately.

**Terminals.** `PtyManager` keeps a rolling 64 KB tail per session plus its current size, and holds
one tap that the bridge registers at startup (`bridge::attach`). On `sub` the phone gets the tail
replayed in 16 KB chunks (so it paints a screen at once) and then live coalesced output; keystrokes
come back as `in` and go straight to `PtyManager::write`. The phone renders at the *desktop's*
cols/rows and scales to fit — it never resizes the PTY, which would reflow the desktop's own view out
from under whoever's sitting at it.

**Nothing here may stall a terminal.** The tap runs on the PTY coalescer thread, so each peer has a
bounded outbound queue and a full one **drops frames** rather than applying backpressure. A phone on
bad wifi degrades its own mirror and nothing else.

**Anything else** (launch a pane, wake a space, git changes/diff/commit, run an automation) is a
generic `req` → Tauri event → `mobileBridge.ts` handler → `bridge_reply`, so the phone reuses the same
`src/api` wrappers the UI does and Rust stays a relay.

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
`settings`, `loops`, …) serialize their state into this.

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
