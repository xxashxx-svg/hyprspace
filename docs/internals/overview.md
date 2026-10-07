# Architecture

One process. `apps/hyprspace/src/main.rs` fixes the process environment, starts the engine, opens
one GPUI window with `ui::Root`, and calls `Engine::shutdown` on quit.

```
ui (GPUI, main thread)  --Command-->  engine (own tokio runtime)  --> harness, PTYs, git, disk
                        <--Event----
```

## The channel

`crates/proto` defines `Command` (UI to engine) and `Event` (engine to UI) as plain serde enums,
carried in-process by two unbounded queues. Commands are fire and forget; a failure comes back as
an event. Each message names the session it is about, or for app requests (state, agents, the
resume list, clones, folder, usage, skills and update work) a request id or the path it is about.

- **One stream, not a channel per session.** The UI drains one `Events` receiver and routes each
  event by its id. One stream keeps ordering simple.
- **Commands and events, not request and response.** When the UI needs an answer it sends a command
  and handles the event that names it. No second transport.
- **Only the binary sees both sides.** `ui` depends on `proto` and `theme`, never `engine`;
  `proto` and `engine` never depend on GPUI. A headless engine later means serializing the same
  enums over a socket, and the proto tests already round-trip every variant through JSON.
- `Engine::shutdown` is a direct call, not a command, because it must finish before the process
  exits: it kills every PTY and drops the runtime, which kills each structured session's CLI.

## The engine

A two-worker tokio runtime. The command loop takes one command at a time, so keystrokes reach a
PTY in the order they were typed. Slow requests (`--version` checks, the resume-list scan,
`git clone`, folder and git work) run on the blocking pool and answer with their own event, so
typing queued behind them isn't held up. `SaveState` stays on the loop so saves land in order.

## Startup environment (`engine/src/env.rs`)

It runs first, before any thread exists, because every shell, CLI and git child inherits it:

- **PATH is rebuilt.** An app started from Explorer, Finder or the Dock gets a thin PATH. The
  engine rebuilds the user's real one (the registry on Windows, a login shell on macOS) so
  `claude`, `codex`, Homebrew and npm globals resolve.
- **Claude's session markers are dropped.** If HyprSpace was started from inside a Claude Code
  session, its markers would leak into every child and confuse the CLIs we start.
- **Ctrl+C is taken back.** A parent that started us with Ctrl+C ignored passes that on to every
  child, so Ctrl+C would reach PowerShell's prompt but never stop the command under it.

## Saved state

`proto::state::AppState` (spaces, threads with the `Launch` each resumes with, the composer's
picks, appearance) is the UI's shape; the engine writes it whole to
`~/.hyprspace/native/state.json`. `HYPRSPACE_STATE_DIR` points a build elsewhere.

- **The store** (`persist::Store`) is single-writer and crash-safe: temp file, fsync, atomic
  rename. It tells "absent" from a read error, and after a real read error it refuses to save, so
  a bad disk moment can't wipe the sidebar. A file that won't parse is moved aside as
  `<name>.corrupt-<ts>.json` and the default comes back.
- **Names on disk** (state files, journals) are reduced to a safe token so they can't leave their
  folder.
- **Old state must load.** A removed field is skipped by serde; a removed enum variant needs a
  rewrite before parsing (`requests::without_gemini`), or the whole file fails and the app starts
  clean.
- **The Tauri app's state** (`~/.hyprspace/v2`) is read once, never written, when
  `native/state.json` doesn't exist (`legacy.rs`, tested against `engine/tests/fixtures/tauri-v2`).
  Projects become spaces, each agent pane a terminal thread on the conversation it was on, with
  nothing launched until it is opened, so a Tauri app still running never shares a live
  conversation with this one.

## GPUI

- **Upstream Zed, one pinned commit** for `gpui`, `gpui_platform` and `gpui_tokio`, set once in the
  root `Cargo.toml`. Every Zed crate it pulls in is Apache-2.0 at that commit. Zed's GPL crates
  (`editor`, `ui`, `theme`, `markdown`, `terminal_view`) stay out of this MIT repo, which is why
  the viewer's editor is our own. Bumping GPUI means changing the rev and rechecking that license
  list; Zed's own `[patch.crates-io]` lines don't apply to us, so copying one over is the first
  thing to try when a bump fails. zeron's fork (`zeronsh/zui`) adds blur and edge fades if we ever
  want them.
- **Layout runs every frame for every uncached view.** One terminal's output used to redo the
  whole sidebar's layout (18 ms of a 20 ms frame at 179 Hz). Big views are embedded with
  `.cached(style)` (terminals in `workbench/frame.rs`, the sidebar in `root/render.rs`) and must
  `cx.notify()` on anything that changes how they look, focus included. Hover styles come from the
  previous frame's hit test.
- **One title row.** The window has no system title bar. `ui/src/root/titlebar.rs` draws one 40px
  row whose empty stretch is a `WindowControlArea::Drag`, which Windows hit-tests to move and snap
  the window. Anything clickable in it must `.occlude()`, or the press drags the window.
