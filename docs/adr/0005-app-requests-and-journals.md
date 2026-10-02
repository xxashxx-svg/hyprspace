# ADR 0005: App requests on the channel, saved state, and thread journals

- Status: Accepted
- Date: 2026-10-02

## Context

Phase 4 puts the real shell on the channel. The sidebar needs its spaces and threads saved, the
composer needs the installed agents, their catalogs, the resume list and clones, and a thread
reopened after a restart has to show its transcript and pick its conversation back up. ADR 0002
says every message names a session, and that a request the UI waits on can be a command and an
event without a new transport.

## Decision

**App requests are commands with their own answer events, and carry no session id.**
`LoadState` / `State`, `SaveState`, `LoadAgents` / `Agents`, `ListResumable` / `Resumable` and
`Clone` / `CloneProgress` / `Cloned` are not about a session, so they don't pretend to be. A clone
carries a `request` number so its progress lands on the composer that asked. The slow ones (the
`--version` checks, the transcript scan, `git clone`) run on tokio's blocking pool, so keystrokes
queued behind them are not held up. `SaveState` runs on the command loop so saves land in order.

**The UI owns the saved state's shape; the engine stores it.** `proto::state::AppState` holds the
spaces, their threads (each with the `Launch` it resumes with), the composer's picks and the
appearance. The engine writes it whole to `~/.hyprspace/native/state.json` through the existing
`Store`. A file that won't parse is moved aside and the default comes back; after a real read
error the engine refuses to save, so a bad disk moment can't wipe the sidebar.

**Each structured thread has a journal.** `OpenStructured` names a journal (`thread-<id>`). The
engine appends every prompt, answer and run event to `journals/thread-<id>.jsonl`, joining
streamed text into one line per reply. `LoadJournal` reads it back, and the transcript replays it
through the same calls it uses for live events, then marks whatever was live as over. The CLI's
own thread id is saved in the thread's `Launch.resume` when `Started` arrives, so the first
prompt after a restart reopens the conversation instead of starting a new one.

**Approvals answer with `Answer::{Allow, AllowAlways, Deny}`.** `RunEvent::Approval` says whether
"always allow" is on offer. Claude offers it when its `can_use_tool` request carries
`permission_suggestions`; answering hands those rules back as `updatedPermissions`. Codex's
command and file-change approvals both take `acceptForSession`.

## Why not

- **The UI writing its own files**: the UI reaches the engine only through the channel, and the
  engine already has a crash-safe store.
- **Reading the CLIs' own transcripts for history**: Claude's and Codex's files differ in shape
  and change between versions. Our journal is the transcript as we drew it. A conversation picked
  from the resume list has no journal, so its thread starts with a note saying its earlier
  messages stay in the agent's own history.
- **A journal per session id**: session ids live as long as the engine; threads outlive restarts.

## Consequences

- Journals grow with each thread and are not trimmed yet. Removing a thread leaves its journal.
- A crash loses the reply that was still streaming, and nothing else.
- A headless engine later would serve the same commands; nothing here is in-process only.
