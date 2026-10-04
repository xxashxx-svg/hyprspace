# ADR 0014: Following an agent started by hand in a terminal

- Status: Accepted
- Date: 2026-10-04

## Context

A terminal thread's row showed the agent HyprSpace launched in it, and its live state came from
the hooks file that launch passed with `--settings` (ADR 0006). Stopping that agent with Ctrl+C
and starting `claude` or `codex` again by hand left the row on the old agent and model, with no
working state or subagents: the new process had no hooks, and nothing looked at what actually
ran.

## Decision

**A watcher reads what runs under each shell.** Every two seconds the engine reads the processes
under each terminal's shell (`running.rs`, on `sysinfo`, which GPUI's screen capture already
builds) and finds the agent nearest the shell: `claude`, `codex` or `gemini` by program name,
or by script path when run through node. A change sends `Event::TerminalAgent`; the thread takes
on that agent and the model its command line names, so its row and a restart follow it. When
the agent quits, the thread keeps the last one and its working state ends.

**Every shell's `claude` brings the session's hooks.** Each terminal gets its hooks file in
`HYPRSPACE_CLAUDE_SETTINGS`. On Windows PowerShell starts with `-NoExit -Command` defining a
`claude` function that runs the real one with `--settings` from that variable, unless the command
names settings itself; it is defined after the user's profile, so it is the one that runs. On
macOS a `claude` script in `~/.hyprspace/agent-hooks/bin` goes first on PATH and does the same.
Claude's hooks then carry its conversation id and, from the status line, its model, so a
`/model` switch shows too.

## Why not

- **Hooks in `~/.claude/settings.json`:** every Claude on the machine would run them, inside
  HyprSpace or not.
- **A `claude.cmd` shim on Windows:** `cmd /c` asks "Terminate batch job?" on Ctrl+C.
- **Reading transcripts instead of hooks:** polled, so laggy, and it can't see a permission wait.

## Consequences

- On macOS a shell profile that puts another folder ahead on PATH bypasses the script; the
  watcher still shows the agent and model, but not its live state.
- A thread where `claude` was ever started by hand comes back as a Claude thread after a restart,
  on the conversation it last ran.
