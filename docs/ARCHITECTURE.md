# Architecture

How the non-obvious subsystems work. For the high-level map and the constraints, see
[../CLAUDE.md](../CLAUDE.md). The words (space, thread, session, run, harness) are defined in
[CONTEXT.md](./CONTEXT.md), and each decision's reasons are in [adr/](./adr/).

## Topology

One process. `apps/hyprspace/src/main.rs` fixes the process environment (`engine::env::prepare`),
starts the engine, opens one GPUI window with `ui::Root`, and calls `Engine::shutdown` on quit.

```
ui (GPUI, main thread)  --Command-->  engine (own tokio runtime)  --> harness, PTYs, git, disk
                        <--Event----
```

- **The channel** (`crates/proto`) is two unbounded queues: `Command` in, `Event` out, plain serde
  enums. The UI holds a `Client` and drains one `Events` stream, routing each event to the view it
  names. Commands are fire and forget; a failure comes back as an event (ADR 0002).
- **The engine** runs a two-worker tokio runtime. Its command loop takes one command at a time, so
  keystrokes reach a PTY in the order they were typed. Slow requests (`--version` checks, the
  resume-list scan, `git clone`, folder and git work) go to the blocking pool and answer with
  their own event, so typing queued behind them isn't held up (ADR 0005).
- **App requests** that aren't about a session (`LoadState`, `SaveState`, `LoadAgents`,
  `ListResumable`, `Clone`, and the `Folder`, `Usage`, `Skills` and `Update` families) carry a
  request id or the path they're about instead of a `SessionId`.
- **Only the binary sees both sides.** `ui` depends on `proto` and `theme`, never on `engine`;
  `proto` and `engine` never depend on GPUI. A headless engine later means serializing the same
  enums over a socket; the proto tests already round-trip every variant through JSON.

## Startup and environment (`engine/src/env.rs`)

Runs first in `main`, before any thread exists, because it mutates the process environment that
every shell, CLI and git child inherits:

- **PATH rebuild.** An app started from Explorer, Finder or the Dock gets a thin PATH. The engine
  rebuilds the user's real one (the registry on Windows, a login shell on macOS) so `claude`,
  `codex`, Homebrew and npm globals resolve.
- **Claude session markers dropped.** If HyprSpace itself was started from inside a Claude Code
  session, the markers it set (`harness::SESSION_ENV`) would leak into every child and confuse the
  CLIs we start.
- **Ctrl+C taken back.** A parent that started us with Ctrl+C ignored passes that on to every
  child, so Ctrl+C would reach PowerShell's prompt but never stop the command under it.

## Structured sessions (`crates/harness`)

One `Harness` per agent starts a `Session`, a tokio task that owns the CLI process and its stdio:

```rust
Harness::start(Launch, Emit) -> io::Result<Session>
Session::send(Prompt)            // starts a run, or steers the live one
Session::interrupt()             // ends the live run as Interrupted, the session stays open
Session::answer(request, Answer) // Allow, AllowAlways or Deny for a RunEvent::Approval
drop(session)                    // kills the CLI
```

- **Claude** runs the user's own `claude` with `--print --input-format stream-json --output-format
  stream-json --verbose --include-partial-messages --replay-user-messages
  --permission-prompt-tool stdio`. `--verbose` is required for stream-json output. `can_use_tool`
  control requests become approvals.
- **Codex** runs its app-server and talks JSON-RPC over stdio (`thread/start`, `turn/start`,
  `turn/steer`, `turn/interrupt`); command and file-change approval requests become approvals.
- **One `send`, not send and steer.** Only the harness knows without a race whether a run is live,
  so a prompt sent mid-run steers and one sent between runs starts a run.
- **Exactly one `Finished` per run.** A Claude `now` steer cuts the current turn with a `result`.
  The harness writes each steer with a `uuid` and holds a `result` that arrives before the steer is
  echoed back, releasing it after 5 seconds of quiet if the CLI absorbed the steer. Steers go as
  `priority: "next"` while a tool call is open, because `now` aborts the tool.
- **Interrupts give up after 5 seconds**: the CLI is killed and the session reports `Failed`.
- **Approvals are never auto-answered.** The permission mode (`Plan`, `Ask`, `Auto`, `Bypass`)
  maps to Claude's `--permission-mode` and Codex's `approvalPolicy` and `sandbox` (table in ADR
  0004). Bypass is the way to skip the questions.
- **Resume pins Claude's folder.** `claude --resume <id>` only finds a conversation from the folder
  it started in, so the harness reads the `cwd` recorded in `~/.claude/projects/*/<id>.jsonl` and
  spawns there. Codex threads carry their own folder.
- **Subagents report under their call.** Claude frames with a `parent_tool_use_id` become
  `SubagentTool`, `SubagentToolDone` and `SubagentText` keyed by the Agent call, never folded into
  the main reply. A background subagent can finish after the run; the CLI then takes a turn of its
  own, reported as `Woke` followed by a normal run.
- **Tests** drive both adapters through `fixtures/fake_cli`, a Rust fake of both CLIs built as a
  bin of the harness crate so it runs on both CI runners. `examples/live.rs` runs one real
  session, which is where a protocol change in a CLI shows up.
- **Catalog.** `catalog.rs` holds the models and effort levels per agent, plus Codex's own
  `~/.codex/models_cache.json` when it exists.

## Journals and saved state (`engine/src/journal.rs`, `persist.rs`, `requests.rs`)

- **State.** `proto::state::AppState` (spaces, threads with their `Launch`, grids, composer picks,
  appearance, `intro_seen`, `seen_version`) is the UI's shape; the engine writes it whole to
  `~/.hyprspace/native/state.json`. `HYPRSPACE_STATE_DIR` points a dev build elsewhere.
- **The store** (`persist::Store`) is single-writer and crash-safe: temp file, fsync, atomic
  rename, under one poison-tolerant lock. `load` tells "absent" (`Ok(None)`) from an IO error
  (`Err`); after a real read error the engine refuses to save, so a bad disk moment can't wipe the
  sidebar. A file that won't parse is moved aside as `<name>.corrupt-<ts>.json` and the default
  comes back. Names are reduced to a safe token so they can't leave the folder.
- **Journals.** Each structured thread appends every prompt, answer and run event to
  `journals/thread-<id>.jsonl`, joining streamed text into one line per reply. After a restart the
  transcript replays it through the same calls it uses for live events. The CLI's own thread id
  lands in the thread's `Launch.resume` when `Started` arrives, so the next prompt reopens the
  conversation. A crash loses only the reply that was still streaming. Journals aren't trimmed yet.
- **The Tauri app's state** (`~/.hyprspace/v2`) is imported once, read only, when
  `native/state.json` doesn't exist (`legacy.rs`, ADR 0012): projects become spaces, open spaces'
  folders become spaces of their own, theme, agent picks, permission, widths and
  `lastSeenVersion` come along. Panes don't become threads; the resume list covers them.

## Terminal sessions (`engine/src/pty.rs`, `terminal.rs`, `hooks.rs`, `ui/src/terminal/`)

- **PTYs.** `PtyManager` spawns a bare shell through `portable-pty` (ConPTY on Windows) with
  `TERM` set, because GUI-launched apps inherit none and CLIs then drop color. A reader thread
  feeds a bounded channel (real backpressure, no dropped bytes) into a **coalescer**: the first
  bytes after a quiet moment go out at once, so keystroke echo is instant, and a sustained stream
  batches to one frame (16 ms) or 16 KB. Output crosses the channel as `TerminalOutput` batches.
- **Lifecycle.** `child.wait()` runs off-thread, then `TerminalExit` is sent. Killing a session
  drops its master PTY off-thread, because closing a pseudoconsole can block until the process
  tree detaches. `kill_all` runs from `Engine::shutdown` on quit: orphaned `OpenConsole.exe` hosts
  busy-spin at about 8% CPU each. Locks recover from poisoning, so one panic can't brick PTY I/O.
- **Launching agents.** `terminal.rs` builds `claude ...`, `codex ...` or `gemini ...` from fixed
  flags and catalog ids (anything past a plain token is quoted) and the PTY types it into the shell
  once it first prints, so the user's profile and PATH apply. User text never enters the command
  line. Claude gets the composer's prompt typed in when its status line first reports, which is
  when its TUI reads input. Codex and Gemini have no such signal, and keys typed on a timer once
  answered Codex's "update available" dialog, so they start with the prompt as their own argument,
  read from `HYPRSPACE_PROMPT`, which no shell re-parses (ADR 0006).
- **A Claude thread owns its conversation id.** The UI picks a UUID when it creates the thread; the
  engine passes `--session-id <id>` the first time and `--resume <id>` once Claude's transcript
  for that folder exists.
- **Hooks.** Each Claude terminal session gets a scoped `--settings` file whose hooks
  (`UserPromptSubmit`, `Stop`, `SessionStart`, `Notification`, `SubagentStop`, `PreToolUse`,
  `PostToolUse`) and status line re-invoke our own binary: `hyprspace agent-hook <port>
  <session>` or `hyprspace status-line <port> <session>`. That short-lived process reads the
  payload from stdin and posts it to a loopback listener on an OS-picked port, so the sidebar
  shows Working, Needs your answer and Done as they happen. Session ids are checked against
  `[A-Za-z0-9_-]` because they go into a command and a path. Approving a permission produces no
  hook of its own, which is why `PostToolUse` is wired: it's what ends a "needs your answer" state.
  `HYPRSPACE_DEBUG_HOOKS=1` logs payloads; it's off by default because they hold prompts. Codex and
  Gemini rows show no live state (no hooks).
- **The emulator lives in the UI.** `ui/src/terminal/` folds bytes through `alacritty_terminal`
  (selection, scrollback, find, modes, cursor), answers terminal queries itself, paints the grid on
  a canvas with block and line characters drawn as rectangles, and reads text through GPUI's input
  handler, so IME, dead keys and AltGr work. Keys with a meaning are encoded in `keys.rs`; links
  and `path:line:col` open on ctrl+click; pasted bitmaps are saved as PNGs and pasted as paths.
  `Root` keeps one view per thread, so moving a pane never rebuilds an emulator.

## Panes, dock and viewer (`ui/src/panes/`, `dock/`, `viewer/`, `engine/src/folder.rs`)

- **Tiles, not tabs.** Each space's `Grid` (saved in `AppState`) holds the panes on screen in
  layout order, the focused and maximized one, the preset per pane count, and dragged track sizes
  per layout. Presets are the Tauri app's grid.ts, as data in `layout.rs`. Panes sit on fractions
  of the frame, so a dragged boundary needs no measuring; a boundary a pane spans stays fixed.
- **The grid is a view over the threads.** Clicking a sidebar row puts that thread in the focused
  pane; ctrl+click adds a pane. Closing a pane leaves the thread running and in the sidebar. A
  removed or archived thread drops out of the grid when it is drawn, so saved grids never need
  tidying (ADR 0007).
- **Frames tell panes apart.** Tiled panes each get a header and the focused one an accent
  border. A pane alone has no accent border. A structured thread alone has no frame or header at
  all: it fills the main area, and the bar above shows its agent, title, folder and model.
- **One viewer pane per space** shows a file or one file's diff, read only, colored by
  `crates/syntax` (tree-sitter) with the theme's terminal palette. Reads are capped at 2 MB; past
  that, or for media, the file goes to the user's editor.
- **Folder requests** ride `Command::Folder` / `Event::Folder`: listings, reads, git status, stage,
  commit, push, diffs, the installed openers, and open-in. Git calls take one lock, because two
  quick ticks would otherwise race for git's `index.lock`. Change paths are relative to the repo
  root, so stage and diff work when the dock follows a subfolder. The dock polls git every four
  seconds while it is out.
- **Opening things outside the app** (`open.rs`): VS Code or Cursor with `--goto file:line:col`,
  Explorer or Finder for folders. Only paths that exist reach a launcher. `HYPRSPACE_OPEN_LOG`
  appends each launch's command line to a file instead of running it.

## Usage (`engine/src/usage/`, `ui/src/usage/`)

- **Live limits** (`live.rs`) read the token each CLI already stores and send it to that
  provider's own usage endpoint, per CLAUDE.md rule 1. The engine owns the floor: a request inside
  180 s (Claude) or 60 s (Codex) is answered from the last reading, and 429 or 5xx backs off. So no
  view can ask faster by mistake. `HYPRSPACE_USAGE_FIXTURES` reads files instead of the network.
- **Free sources.** The hook listener tees Claude's status line; `status.rs` turns its
  `rate_limits` into the same shape the endpoint gives, sent per session. Codex's session files
  are read only while its live reading has no windows. `local.rs` aggregates each CLI's own files
  for Settings' activity view, display only.
- **One entity** (`ui::usage::Limits`) asks on a 30-second tick when each provider is due and holds
  every reading, so the ring above the panes and Settings never disagree (ADR 0009).

## Updates and installers (`crates/update`, `engine/src/update.rs`, `scripts/package-*`)

- **One feed, one key.** The app reads `releases/latest/download/latest.json`
  (`{ version, notes, pub_date, platforms }`, written by `scripts/ci-build-latest.mjs`) and checks
  each download's minisign signature against the key the Tauri app shipped with. The feed URL and
  key can only be swapped at compile time (`option_env!`), never at run time.
- **Only an installed copy updates.** A copy next to the installer's `uninstall.exe`, or inside a
  `.app`, is installed; `cargo run` and `target/release` builds answer `Unmanaged` and never check.
- **Windows.** The engine downloads, verifies and starts the NSIS installer with the Tauri updater's
  own arguments (`/P /R /UPDATE /ARGS`) and the UI quits. The installer
  (`apps/hyprspace/package/windows/installer.nsi`) keeps the Tauri installer's identity: same
  per-user registry keys and folder, one Apps entry, `hyprspace-tauri.exe` deleted and its
  shortcuts pointed at `hyprspace.exe`. It closes only processes whose image is in its own install
  folder, with `WM_CLOSE` first so the app kills its PTYs the normal way (ADR 0010).
- **macOS.** The engine unpacks the `.app.tar.gz` over the bundle and a detached shell reopens it
  once this process is gone. `scripts/package-macos.sh` assembles the bundle, signs it (Developer
  ID when the secrets exist, ad hoc otherwise) and builds the dmg.
- **Leftover downloads** from both updaters are swept from the temp folder at launch, with retries
  while the installer that just ran still holds its file (ADR 0011).
- **The UI** checks on launch, every 6 hours and on focus after 15 minutes, shows a corner card with
  "Restart and update", and shows What's new from the bundled `docs/CHANGELOG.md` on the first
  launch of a new version. Settings, About shows the same state and can show What's new again.
- **CI.** `release.yml` signs with the `TAURI_SIGNING_PRIVATE_KEY` secret through
  `npx @tauri-apps/cli signer sign` (the key format is Tauri's) and checks every signature with
  `cargo run -p hyprspace-update --example verify` before uploading. `upgrade-test.yml` builds the
  v0.21.1 Tauri app from its tag and watches its real updater install the GPUI app on both
  platforms.

## Settings (`ui/src/settings/`)

- **Views are tabs over `AppState`.** General, Appearance, Agents, Usage, Skills, Shortcuts and
  About, listed once in `TABS`, which the command palette reads too. A control edits
  `Root::state` and saves through `Root::save`; General's agent, start mode and permission and
  Agents' model and effort are the composer's own picks (`ComposerPrefs`), so either place changes
  both.
- **The terminal font is read where it paints.** `Appearance.terminal_font` and
  `terminal_font_size` are copied into a thread local whenever the theme applies, and
  `ui/src/terminal/paint.rs` reads it every frame, so open terminals reflow on the next frame.
- **Shortcuts is a read-only list.** It mirrors the real bindings in `palette/`, `panes/`,
  `input/` and `terminal/`; a changed binding changes there and in `settings/shortcuts.rs`.

## Provider status (`engine/src/providers.rs`)

Runs `<cli> --version` (arguments passed separately, never a shell string) and reads
`~/.claude.json`, `.credentials.json` and Codex's `auth.json` for **display-only** account and plan
fields. A JWT is base64-decoded **without verification**, only to show an email or plan: no trust
decision, never forwarded.
