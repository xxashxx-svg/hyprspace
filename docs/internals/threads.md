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
- `proto::Grid` is gone. A state file that still has `grid` loads; serde skips the field. `Pane`
  stays: a dragged sidebar row carries one, and the viewer shows `Pane::File` and `Pane::Diff`.

## A space is a folder

There is one kind of space, one folder. "New thread" asks for a folder, reuses that folder's space
or adds one. Every thread runs in a folder anyway, and `claude --resume` only works in the folder a
conversation started in, so a space with no folder (the Tauri app's open spaces) was a second
place to put a thread that the dock, the Open menu and resume all had to special-case. `Space.cwd`
is still an `Option` so old state loads.

## Settle and snooze

Ash runs many threads and clears finished ones all the time. Archiving only hid a thread while its
agent kept running, and a Claude process holds a few hundred MB, so a day of work left dozens of
idle agents behind. Settle and snooze replaced it.

- **Settling frees the session.** An idle settled thread's terminal and agent close, and the
  conversation resumes when the thread is opened again. A busy one finishes its turn first, and one
  waiting on an approval keeps its question. A plain shell keeps its terminal, since it has
  nothing to resume and may be running a dev server.
- **Untouched threads settle by themselves** after three days by default (Settings, General).
  "Touched" means a turn started or ended, or the thread came back from Settled or Snoozed. Only
  opening it doesn't count, so the age on its row stays the time of its last activity. The thread
  on screen or at work never settles by itself.
- **Snooze** hides a thread until a time or until its agent finishes. A snoozed thread keeps its
  session and wakes at the top of the list marked new.
- Opening a settled or snoozed thread doesn't bring it back; sending it a message does. Settling
  and snoozing show a toast with Undo for five seconds.
- Saved `archived` threads load as settled, and an archived space loads with its threads settled.
