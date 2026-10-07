# Glossary

The words HyprSpace uses. Code, UI copy and docs use exactly these. A new idea that needs a name
gets it here first.

## Where work happens

- **Space**: one folder, with the threads that run in it. The sidebar lists threads, each tagged
  with its space's initials and name, not a section per space. Code that adds a space may still
  say `add_project`.
- **Worktree**: a separate git checkout under `~/.hyprspace/worktrees/` so an agent can work on its
  own branch (`hs/<name>`) without touching the main checkout.

## What the user works with

- **Thread**: one conversation in a space, one sidebar row with its live status. What the user
  names, settles, snoozes, reopens and resumes.
- **Settled**: a thread moved to the Settled shelf at the bottom of the sidebar, by hand or after
  sitting untouched. Its conversation is kept, and sending it a message brings it back.
- **Snoozed**: a thread hidden until a time, or until its agent finishes its turn, then back in the
  list marked new.
- **Session**: the live process behind a thread, identified by a `SessionId`. Two kinds:
  - **Structured session**: an agent CLI driven over its machine protocol (Claude's stream-json,
    Codex's app-server) and drawn as a transcript.
  - **Terminal session**: a PTY running a shell, for plain shells and for running an agent CLI
    interactively.
- **Run**: one prompt in a structured session, from sending it until the CLI reports a result.
  Steering adds to the current run; interrupting ends it.
- **Steer**: a prompt sent while a run is live. It joins that run.
- **Approval**: the agent asking yes or no before it runs a tool. The run waits for the answer.
- **Subagent**: an agent a run starts with Claude's Agent tool to do one task and report back,
  shown as one card in the transcript. One in the background can outlive the run.
- **Thread id**: the id a CLI gives its conversation (Claude says session id, Codex thread id).
  Resuming takes it.
- **Transcript**: the drawn record of a structured session: prompts, replies, tool calls,
  approvals and diffs.
- **Journal**: the engine's file of a thread's prompts, answers and run events
  (`journals/thread-<id>.jsonl`), replayed to rebuild its transcript after a restart.
- **Main area**: where the one thread on screen shows, or a space's composer.
- **Viewer**: the card over the window that shows a file or one file's diff. A text file in it can
  be edited and saved.
- **Lightbox**: an image shown over the window, zoomable and movable.
- **Dock**: the right panel (Ctrl+Shift+G) with the file tree and the git tab for the folder of the
  thread on screen.
- **Opener**: an app that opens a folder: a code editor, or Explorer or Finder.
- **Agent**: a coding CLI the user has installed and signed in to (`claude`, `codex`). Also a
  **provider** where the subject is the account behind it (sign-in, plan, usage). Settings'
  Activity also reads two providers it can't start, OpenCode and Grok.
- **Resume list**: the conversations an agent saved on disk for a folder, which the composer
  offers to reopen.
- **Command palette**: the searchable list of commands, threads and terminal text (Ctrl+K or
  Ctrl+Shift+P, anywhere).
- **Intro**: the steps shown once on a first run with no spaces, replayable from the palette.
- **Skill**: a Claude skill (`.claude/skills/<name>/SKILL.md`) or slash command
  (`.claude/commands/<name>.md`), from the project or the user's home.

## How the app is built

- **Engine**: everything that isn't drawing: sessions, PTYs, git, providers, usage, saved state.
  No GPUI.
- **UI**: the GPUI app. It reaches the engine only through the channel.
- **Channel**: the typed boundary between them (`hyprspace-proto`). The UI sends **commands**, the
  engine sends **events**.
- **Harness**: the adapter that drives one agent CLI over its machine protocol and turns its output
  into run events.
- **Hook**: a Claude Code lifecycle hook that re-invokes our binary to report a terminal session's
  state to the engine over loopback.

## Usage

- **Live limits**: the account's real rate-limit windows, from the provider's usage endpoint.
- **Window**: one rate limit (5 hours, a week): how full it is and when it resets. Claude calls its
  5-hour window "session", and the meter keeps Claude's word.
- **Activity**: what each agent's own files on disk say about tokens and sessions. Display only.

## Words to avoid

- **Chat**: say structured session, or transcript for what's on screen.
- **Workspace** for a space: it's the Cargo workspace in this repo.
- **Turn** for a run: Claude's result counts several turns (`num_turns`) inside one run.
