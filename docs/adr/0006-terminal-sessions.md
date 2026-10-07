# ADR 0006: Terminal sessions run agents by typed command, and the engine owns the launch

- Status: Accepted; the Gemini part superseded by ADR 0017
- Date: 2026-10-02

## Context

REWRITE.md keeps the terminal as a first-class session type: plain shells, and agent CLIs run
interactively, because interactive Claude Code keeps the plan's full limits. The Tauri app typed
the launch command into the shell from the webview, wired Claude's hooks for the sidebar, and
resumed each pane's own conversation. The GPUI app needs the same, with the UI reaching the
engine only through the channel (ADR 0002).

## Decision

**The engine builds and types the launch command.** `Command::OpenTerminal` carries an optional
`Launch` (agent, model, effort, permission, resume) and prompt. `engine/src/terminal.rs` turns it
into `claude ...`, `codex ...` or `gemini ...` from fixed flags and catalog ids, and the PTY types
it in once the shell first prints, as the Tauri app did, so the user's profile and PATH apply.
The engine does it because the hooks file, the hook listener's port and the transcript check
all live there.

**User text never goes into the command line.** Claude gets the composer's prompt typed in when
its status line first draws, which is when its TUI reads input. Codex and Gemini have no such
signal, and typing on a timer is not safe: during testing, the Enter meant for a Codex prompt
landed on Codex's "update available" dialog and installed an update. So those two start with the
prompt as their own argument, read from the `HYPRSPACE_PROMPT` environment variable
(`$env:HYPRSPACE_PROMPT` in PowerShell), which no shell re-parses.

**A Claude thread owns its conversation id.** The UI picks a UUID when it creates the thread and
saves it in `run.resume`; the engine passes `--session-id <id>` the first time and
`--resume <id>` once Claude's transcript for that folder exists, so a restart reopens the same
conversation in the folder it started in.

**Gemini is an `Agent`.** `Agent::structured()` says which agents a harness can drive; Gemini is
false, so the composer starts it in a terminal and `harness::for_agent` returns None for it.

**Ctrl+C is taken back at startup.** A process started with Ctrl+C ignored passes that on to its
children, which made Ctrl+C reach PowerShell's prompt but never stop a running command. `env.rs`
clears the flag before any shell starts.

**Typing goes through GPUI's input handler.** Keys with a meaning (Enter, arrows, Ctrl and Alt
combinations) are encoded in `keys.rs`; everything that types text, including IME commits, dead
keys and AltGr, arrives as text through `EntityInputHandler`, and IME composition is drawn at the
cursor until it commits.

## Why not

- **The UI building the command:** it would need the hooks port and the transcript check, which
  are engine state.
- **Claude's own `claude "prompt"` start:** the status-line signal already works and keeps the
  prompt out of the command line.
- **Keeping the Tauri app's 4.5 second typing delay for Codex:** it is what triggered the update.

## Consequences

- A Claude row that waited on a permission and was declined with Esc stays "Needs your answer"
  until the next prompt: Claude sends no hook for an interrupt. The Tauri app behaved the same.
- Codex's managed app-server daemon outlives a closed terminal session by design; the shell and
  the Codex TUI die with it.
- A ctrl+clicked file opens in VS Code or Cursor at its line, or the OS default for media, until
  the app has its own viewer (phase 6 changes `Root::open_file`).
