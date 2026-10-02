# Domain context

The words the GPUI app uses, defined once. Code, UI copy and docs use exactly these. If a new
idea needs a name, add it here first. Modeled on zeron's `CONTEXT.md`.

## Where work happens

- **Space**: a place sessions live, shown as one section of the sidebar. Either a **project**
  (one folder) or an **open space** (a scratch space whose sessions can each sit in a different
  folder). The Tauri app calls the same thing a workspace in code; the GPUI app says space.
- **Project**: a space tied to one folder.
- **Open space**: a space with no folder of its own.
- **Worktree**: a separate git checkout under `~/.hyprspace/worktrees/` so an agent can work on its
  own branch (`hs/<name>`) without touching the main checkout.

## What the user works with

- **Thread**: one conversation in a space, shown as one sidebar row with its live status. A
  thread is what the user names, archives, reopens and resumes.
- **Session**: the live process behind a thread, identified by a `SessionId`. There are two
  kinds:
  - **Structured session**: an agent CLI driven over its machine protocol (Claude's stream-json,
    Codex's app-server) and rendered as a transcript.
  - **Terminal session**: a PTY running a shell, painted as a terminal grid. Used for plain shells
    and for running an agent CLI interactively.
- **Run**: one prompt in a structured session, from sending it until the CLI reports a result.
  A structured session holds many runs. Steering adds to the current run; interrupting ends it.
- **Steer**: a prompt sent while a run is live. It joins that run instead of starting a new one.
- **Approval**: the agent asking for a yes or no before it runs a tool. The run waits for the
  answer.
- **Thread id**: the id a CLI gives its conversation (Claude calls it a session id, Codex a
  thread id). Resuming a thread takes it.
- **Transcript**: the rendered record of a structured session: prompts, replies, tool calls,
  approvals and diffs.
- **Journal**: the engine's file of a thread's prompts, answers and run events
  (`journals/thread-<id>.jsonl`), replayed to rebuild its transcript after a restart.
- **Pane**: one cell of a space's grid, showing a thread's session or the viewer.
- **Grid**: the panes a space shows, their layout and the sizes the user dragged. Saved with the
  space. A thread opened from the sidebar takes the focused pane's place; ctrl+click adds it.
- **Viewer**: the read-only pane that shows a file or one file's diff. A space has at most one.
- **Dock**: the right panel (Ctrl+Shift+G) with the file tree and the git tab for the focused
  thread's folder.
- **Opener**: an app that opens a folder: a code editor, or Explorer on Windows and Finder on
  macOS.
- **Agent**: a coding CLI the user has installed and signed in to (`claude`, `codex`, `gemini`,
  `opencode`, `grok`). Also called a **provider** where the subject is the account behind it
  (sign-in, plan, usage).
- **Resume list**: the conversations an agent saved on disk for a folder, which the composer
  offers to reopen. `claude --resume <id>` only works in the folder the conversation started in.
- **Skill**: a Claude skill (`.claude/skills/<name>/SKILL.md`) or slash command
  (`.claude/commands/<name>.md`), from the project or the user's home.

## How the app is built

- **Engine**: everything that is not drawing: sessions, PTYs, git, providers, usage,
  persistence. No GPUI. Runs in-process on its own tokio runtime.
- **UI**: the GPUI app. It owns views and input, and reaches the engine only through the channel.
- **Channel**: the typed boundary between UI and engine (`hyprspace-proto`). The UI sends
  **commands**; the engine sends **events**. Every message names the session it is about.
- **Harness**: the adapter that drives one agent CLI over its machine protocol and turns its
  output into run events. One harness per agent (Claude, Codex).
- **Hook**: a Claude Code lifecycle hook that re-invokes our binary and reports a terminal
  session's state (working, waiting, done) to the engine over loopback.

## Usage

- **Local usage**: what an agent's own files on disk say about activity and tokens. Display only.
- **Live limits**: the account's real rate-limit windows, read from the provider's usage
  endpoint with the token its CLI already stores (CLAUDE.md rule 1). Claude is asked at most once
  every 180 seconds.
- **Window**: one rate limit (5 hours, a week), with how full it is and when it resets. Claude
  names its 5-hour window "session", and the meter keeps that label because it is Claude's word.

## Words to avoid

- **Chat**: say structured session, or transcript for what is on screen.
- **Workspace** for a space: it is a Cargo workspace in this repo.
- **Turn** for a run: Claude's stream-json result counts several turns (`num_turns`) inside one
  run.
