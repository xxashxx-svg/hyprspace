# Threads, the sidebar and the main area

## One thread on screen

The main area shows one thread at a time. Clicking a sidebar row shows that thread, and a new
thread or terminal takes the main area. There are no tiles, layouts or "open beside".

- **Why not tiles.** The Tauri app tiled every session of a space. In use, a second pane mostly
  arrived by accident, and Ash asked for the grid to go. The sidebar already does what tiles did:
  every thread is one click away, with its live state on its row.
- Threads off screen keep their sessions running. `Root.views` holds one view per thread, so
  switching never rebuilds a terminal and its emulator.
- A terminal sits in a frame with its header (agent mark, title, model tag, close back to the
  composer). A structured thread runs edge to edge, with its name in the bar above.
- Ctrl+` opens a drawer of shells under a structured thread, one tab each (`workbench/drawer.rs`).
  Their sessions stay out of `Root.sessions`, so their hooks, agents and exits never touch the
  thread. `/login` opens one that runs the agent's own sign-in (`claude auth login`, `codex
  login`), and closes the thread's CLI if it is idle, so the next message starts one that reads
  the new login.
- `proto::Grid` is gone. A state file that still has `grid` loads; serde skips the field. `Pane`
  stays: a dragged sidebar row carries one, and the viewer shows `Pane::File` and `Pane::Diff`.

## Background and glass

- **The background is the engine's copy.** Picking a picture or shuffling the wallpaper folder
  sends it to the engine, which copies it into `backgrounds/` in the state folder, takes its main
  color (oklch, favoring colorful pixels) for the "From wallpaper" theme, and draws the chosen
  effect into a second file. Only the current picture's files stay; the rest are deleted.
- **Glass makes the window's own surfaces see-through** (`hyprspace_theme::glass`) and asks the
  system to blur the desktop behind the window. Anything that paints `colors::bg()` to cover what
  is under it shows through under glass; cover with `surface2` or `surface3` instead.

## A space is a folder

There is one kind of space, one folder. "New thread" asks for a folder, reuses that folder's space
or adds one. Every thread runs in a folder anyway, and `claude --resume` only works in the folder a
conversation started in, so a space with no folder (the Tauri app's open spaces) was a second
place to put a thread that the dock, the Open menu and resume all had to special-case. `Space.cwd`
is still an `Option` so old state loads.

"No project" keeps that rule. It is an ordinary space on `scratch` in the state folder, and each
thread started there gets a new folder of its own inside it, so two threads never write over each
other's files. The folder is the day and a random token, not the prompt: the title arrives after
the first run has started there, and like a worktree's, the folder can't move after that. A terminal opened on the space runs in `scratch`
itself.

## Several at once

Shift+click in a new thread's model menu adds or drops more models the message also goes to. Each
pick is its own thread in its own worktree, so a git project is needed; the first opens and the
rest start in the background. Ctrl+Enter (Cmd on macOS) starts a thread without leaving the new
thread screen. A worktree can be ready after the screen moved on, so `ComposerEvent::Start`
carries the space it was sent from instead of reading the screen.

## Scheduled tasks

A message in a new thread's box can run later instead (the clock beside Send): every hour or six,
daily or weekly at a time, or when CI fails, a pull request opens or an issue gets a label. Each
run is a new thread with the box's project, agent, model, permission and worktree choice. Tasks
live in `AppState.scheduled` and are run by the UI's 15 second tick, so they run only while the
app is open; a clock time missed while it was closed runs once on opening, up to a week back.
Watches ask `gh` every five minutes. A watch's first look only records the newest run or number,
so making one never starts threads for what was already there, and one look starts at most three.

## Handing off

`/handoff`, or the button on a usage-limit line, starts a thread with the other agent in the same
folder and worktree. Neither CLI can read the other's saved conversation, so the new thread's first
message carries it as text (`transcript/handoff.rs`): the first request, then the latest turns that
fit in about 40,000 characters, with tool calls named but not their output.

## Drafts

Text typed in the composer is saved as a draft (`AppState.drafts`) 600 ms after the last change,
with its images, picks and worktree choice, so a restart doesn't lose it. Drafts are rows at the
top of the sidebar, after T3 Code's. A draft started on screen gets its row once the user leaves
it, so the sidebar doesn't change under their typing.

- New thread leaves a draft with text as its row and opens an empty composer. Sending removes it.
- The composer's own space picker moves its draft to the new space. Reaching another space's
  composer from anywhere else (a thread's close button, leaving a settled thread) parks the draft
  instead, so it never changes project behind the user's back.
- A background start (Ctrl+Enter) clears the box before its worktree exists. If the worktree
  fails, the prompt goes back in the box when the box is empty, and becomes a draft row when the
  next prompt is already there. It never overwrites what the user is typing.
- Structured sessions come from Settings, not the draft, when it loads.
- Drafts are for this computer's spaces. Another computer's space ids are handed out per
  session, so a draft saved on one could come back on the wrong space. Its composer keeps the
  text in memory only.

## Settle and snooze

Ash runs many threads and clears finished ones all the time. Archiving only hid a thread while its
agent kept running, and a Claude process holds a few hundred MB, so a day of work left dozens of
idle agents behind. Settle and snooze replaced it.

- **Settling frees the session, once it's known to be idle.** An idle settled thread's terminal
  and agent close, and the conversation resumes when the thread is opened again. A working one
  finishes its turn first, and one waiting on an approval keeps its question. Only a structured
  thread and Claude in a terminal (through its hooks) say whether they're working; Codex in a
  terminal never does, so it keeps running when settled, like a plain shell, which has nothing to
  resume and may be running a dev server.
- **Untouched threads settle by themselves** after three days by default (Settings, General).
  "Touched" means a turn started or ended, or the thread came back from Settled or Snoozed. Only
  opening it doesn't count, so the age on its row stays the time of its last activity. The thread
  on screen or at work never settles by itself.
- **Snooze** hides a thread until a time or until its agent finishes. A snoozed thread always
  keeps its session, working or not, and wakes at the top of the list marked new.
- Opening a settled or snoozed thread doesn't bring it back; sending it a message does. Settling
  and snoozing show a toast with Undo for five seconds.
- Saved `archived` threads load as settled, and an archived space loads with its threads settled.

## Worktree folders

A worktree's folder is `<repo>-hs-<random token>` and never moves, because Claude and Codex keep
their conversations by folder, so a thread resumes only at the path it started in. Only the branch
takes a name: it starts as `hs/<token>` and becomes `hs/<title>` once the thread has a title,
unless it was pushed by then. The title part is its first three words that aren't filler, at
most 24 characters, so a branch fits in the footer and a PR title.

A new worktree starts from its base branch's upstream, fetched first, when the local branch has
nothing the upstream lacks, so it isn't days behind; with local commits of its own it starts from
the local branch, so nothing unpushed is left out. The branch is made with `--no-track`: started
off `origin/main` it would otherwise track it, and a push from the worktree would land on `main`.
Then the repo's `.worktreeinclude` (gitignore patterns, the file Claude Code reads too) picks the
ignored files a checkout leaves behind, such as `.env`, to copy in, and the space's setup command
(composer, branch menu) runs there before the agent starts. A failed command only shows a notice.

A worktree whose threads have all sat settled for the wait in Settings, General (three days by
default) loses its folder, since a Rust checkout carries gigabytes of `target/`. The branch stays,
and the engine records the worktree in `shelved.json`. Before a session, a terminal or a worktree
action opens at that path again, the engine checks the branch out there again. The folder only
goes when no thread on it has a view (so no agent or shell is in it) and `git status` shows
nothing uncommitted or untracked; ignored files such as `.env` go with it. Git for Windows before
2.54 deletes through NTFS junctions while removing a worktree, so on Windows nothing is removed
with an older Git.

## Pull requests

An unsettled thread on a branch (its worktree's, or its folder's when that isn't main or master)
shows that branch's pull request beside it, read every five minutes as one `gh pr list` per repo,
with a dot while its checks fail or run. An open one is preferred over an older closed one for the
same branch. A thread whose pull request goes from open to merged while the app watches settles,
unless it's at work or on screen; one that was merged before stays where the user put it. `#` in a
reply box lists the repo's pull requests and inserts the chosen one's link.

## The screenshot shortcut

Ctrl+Alt+S, from any app, captures the window in front and attaches it to the thread on screen
(`engine/src/snapshot.rs`). It's off by default: a system-wide hotkey takes those keys from every
other app while it's held. The capture copies the window's rectangle off the screen rather than
asking the window to draw itself, which DirectX apps such as Roblox Studio answer with black. It
is Windows only; macOS would need its own hotkey and the screen recording permission.

## Servers a thread started

The title row shows the ports a thread's processes listen on, read every 15 seconds and when a
thread comes on screen (`engine/src/servers.rs`), with a click to open one and a button to stop
it. A port counts only when a shell sits between its process and the session: a CLI's own MCP
servers are its children and some of them listen too (Roblox Studio's MCP does), while an agent's
commands and a terminal's run under a shell. Stop ends only a process inside one of our sessions'
trees, so an id reused by Windows can't stop someone else's.

## The Working shelf

A beta in Settings, General, after T3 Code's working shelf. A thread whose agent works folds into
a Working shelf and comes back to the top of the list when it finishes (after a 2 second hold, so
back-to-back runs don't make it jump) or needs you.

- **Coming back writes `order`, like waking from a snooze.** T3 sorts its whole inbox by return
  time while the shelf is on; that would undo the order rows were dragged into, so only the
  returning thread moves.
- **Busy means working, or subagents still running after the run ended.** Subagents ending
  change no status, so each run event and subagent update looks again. Codex in a terminal and
  plain shells never say they work, so they never go. Threads on another computer move on and
  off but keep the host's order.
- Pinned, snoozed and settled threads stay where they are, and a subagent's thread stays under
  its parent. The phone doesn't show the shelf yet.
