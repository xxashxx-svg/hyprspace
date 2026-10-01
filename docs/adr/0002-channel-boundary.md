# ADR 0002: One command queue in, one event stream out

- Status: Accepted
- Date: 2026-10-02

## Context

REWRITE.md asks for the engine to run in-process behind a typed channel, so a headless mode or
the phone can attach later without rework, and for `proto` and `engine` to never depend on GPUI.
zeron does this with a full RPC layer (`crates/rpc`: typed request, response and stream over a
WebSocket or an in-memory duplex) because it also syncs across devices. We have one window and
one engine today.

## Decision

- `hyprspace-proto` defines `Command` (UI to engine) and `Event` (engine to UI) as plain serde
  enums. Every variant carries the `SessionId` it is about.
- In-process the channel is two unbounded `futures` queues. `proto::Client` wraps the command
  sender; the UI gets one `Events` receiver.
- The engine owns a two-worker tokio runtime. Its command loop handles one command at a time, in
  order, so keystrokes for a terminal session reach the PTY in the order they were typed.
- The UI runs one task that drains `Events` and routes each event to the view for its session.
- The UI crate depends on `proto` and `theme`, never on `engine`. Only the app binary sees both:
  it starts the engine, hands the two channel ends to the UI, and calls `Engine::shutdown` on quit.
- Commands are fire and forget. A failure comes back as `Event::Failed` for that session.

## Why not

- **Request and response pairs** (zeron's RPC): nothing in the UI waits on a reply yet. When
  something does (git status for the dock), it can be a command and an event with the same
  session or a request id, without a new transport.
- **One channel per session**: the UI would need a task per session and the engine a sender per
  view. One stream keeps ordering simple and costs a hash lookup per event.
- **gpui_tokio**: the spike spawned claude through it. With the engine on its own runtime the UI
  never touches tokio, so the dependency is gone.

## Consequences

- Library calls the engine already has (git, usage, skills, providers, sessions) are not on the
  channel yet. Each gains a command and an event when the UI first needs it, never a direct call
  from `ui` into `engine`.
- Terminal output crosses as `Vec<u8>` per coalesced batch, at most one per frame under load, so
  the queue cannot grow faster than the view paints in practice. If a firehose ever outruns the
  view, the Tauri app's pause gate is the thing to bring back.
- A headless engine later means serializing the same enums over a socket. JSON works today
  (the proto tests round-trip every variant); terminal bytes would want a binary encoding then.
- `Engine::shutdown` is a direct call, not a command, because it must finish before the process
  exits: it kills every PTY (ConPTY hosts orphan otherwise) and drops the runtime, which kills
  each structured session's child.
