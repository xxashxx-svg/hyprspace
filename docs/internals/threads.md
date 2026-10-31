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
thread started there gets a new folder of its own inside it, named after its title, so two
threads never write over each other's files. A terminal opened on the space runs in `scratch`
itself.

## Drafts

Text typed in the composer is saved as a draft (`AppState.drafts`) 600 ms after the last change,
with its images, picks and worktree choice, so a restart doesn't lose it. Drafts are rows at the
top of the sidebar, after T3 Code's. A draft started on screen gets its row once the user leaves
it, so the sidebar doesn't change under their typing.

- New thread leaves a draft with text as its row and opens an empty composer. Sending removes it.
- The composer's own space picker moves its draft to the new space. Reaching another space's
  composer from anywhere else (a thread's close button, leaving a settled thread) parks the draft
  instead, so it never changes project behind the user's back.
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
