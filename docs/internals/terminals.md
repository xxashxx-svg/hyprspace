# Terminal sessions

A terminal session is a real PTY running the user's shell, for plain shells and for agent CLIs run
interactively. Code: `engine/src/pty.rs`, `terminal.rs`, `hooks.rs`, `running.rs`, and
`ui/src/terminal/`.

## PTYs

- `PtyManager` spawns a bare shell through `portable-pty` (ConPTY on Windows) with `TERM` set,
  because GUI-launched apps inherit none and CLIs then drop color.
- A reader thread feeds a bounded channel (real backpressure, no dropped bytes) into a coalescer:
  the first bytes after a quiet moment go out at once, so keystroke echo is instant, and a
  sustained stream batches to one frame (16 ms) or 16 KB.
- Killing a session drops its master PTY off-thread, because closing a pseudoconsole can block
  until the process tree detaches. `kill_all` runs from `Engine::shutdown`: orphaned
  `OpenConsole.exe` hosts busy-spin at about 8% CPU each. Locks recover from poisoning, so one
  panic can't brick PTY I/O.

## Launching agents

The engine builds `claude ...` or `codex ...` from fixed flags and catalog ids (anything past a
plain token is quoted), and the PTY types it into the shell once it first prints, so the user's
profile and PATH apply. The engine does it because the hooks file, the hook listener's port and the
transcript check all live there.

- **User text never enters the command line.** Claude gets the composer's prompt typed in when its
  status line first reports, which is when its TUI reads input. Codex has no such signal, and keys
  typed on a timer once answered Codex's "update available" dialog and upgraded it. So Codex starts
  with the prompt as its own argument, read from `HYPRSPACE_PROMPT` (`$env:HYPRSPACE_PROMPT` in
  PowerShell), which no shell re-parses.
- **A Claude thread owns its conversation id.** The UI picks a UUID when it creates the thread; the
  engine passes `--session-id <id>` the first time and `--resume <id>` once Claude's transcript
  for that folder exists. Codex can't be handed an id, so the engine watches its rollouts in the
  folder for the one a new session starts, and the thread runs `codex resume <id>` after a restart.
- Codex's managed app-server daemon outlives a closed terminal session by design; the shell and the
  Codex TUI die with it.

## Live state from hooks

Each Claude terminal session gets a scoped `--settings` file whose hooks (`UserPromptSubmit`,
`Stop`, `SessionStart`, `Notification`, `SubagentStop`, `PreToolUse`, `PostToolUse`) and status
line re-invoke our own binary: `hyprspace agent-hook <port> <session>` or `hyprspace status-line
<port> <session>`. That short-lived process posts the payload to a loopback listener on an
OS-picked port, so the sidebar shows Working, Needs your answer and Done as they happen, plus a
line on what the agent is doing and its running subagents.

- Approving a permission fires no hook of its own. `PostToolUse` is wired because it's what ends a
  "needs your answer" state.
- Session ids are checked against `[A-Za-z0-9_-]` because they go into a command and a path.
- `HYPRSPACE_DEBUG_HOOKS=1` logs payloads. It's off by default because they hold prompts.
- **Interrupts send no hook.** Claude keeps `~/.claude/sessions/<pid>.json` per running session
  with a `status` of `busy`, `waiting` or `idle`. The watcher reads it, and only a change counts:
  busy to idle mid-turn is an interrupt, and a status a beat behind the hooks can't undo a Done.
- Hooks don't go in `~/.claude/settings.json`, because every Claude on the machine would run them.

## Agents started by hand

Every two seconds the engine reads the processes under each terminal's shell (`running.rs`, on
`sysinfo`) and finds the agent nearest the shell, by program name or by script path under node. A
change sends `Event::TerminalAgent`, and the thread takes on that agent and the model its command
line names, so its row and a restart follow it.

So a hand-typed `claude` brings the hooks too, each terminal gets its hooks file in
`HYPRSPACE_CLAUDE_SETTINGS`. On Windows, PowerShell starts with `-NoExit -Command` defining a
`claude` function, after the user's profile, that runs the real one with `--settings` unless the
command names settings itself. (A `claude.cmd` shim would make Ctrl+C ask "Terminate batch job?".)
On macOS a `claude` script in `~/.hyprspace/agent-hooks/bin` goes first on PATH; a shell profile
that puts another folder ahead bypasses it, and the row then shows the agent but not its live
state.

## The emulator

`ui/src/terminal/` folds bytes through `alacritty_terminal`, answers terminal queries itself,
paints the grid on a canvas (block and line characters drawn as rectangles), and reads text through
GPUI's input handler, so IME, dead keys and AltGr work. Keys with a meaning are encoded in
`keys.rs`. Links and `path:line:col` open on Ctrl+click; pasted bitmaps are saved as PNGs and
pasted as paths. The palette's Ctrl+K is bound everywhere, terminals included, so a shell never
sees its kill-line Ctrl+K.

## Image previews

Resting the pointer on an image path or on Claude's `[Image #N]` opens a preview beside it, and
Ctrl+click opens a lightbox. A marker for an image pasted but not yet sent comes from the paste
itself: the view reads Claude's input box as it redraws to learn which number the paste became.
Anything else is asked of the engine (`FindImage`), which looks in Claude's old
`~/.claude/image-cache`, then decodes the image out of the conversation's transcript into
`~/.hyprspace/image-cache`.
