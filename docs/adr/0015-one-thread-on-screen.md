# ADR 0015: One thread on screen; files open over it

- Status: Accepted
- Date: 2026-10-04
- Supersedes: the grid half of ADR 0007

## Context

ADR 0007 brought back the Tauri app's tiled grid: each space kept several panes on screen, with
layout presets, draggable boundaries, drag-to-swap, double-click to maximize, and a file or diff
viewer as one more pane. In use, the second pane mostly arrived by accident. Starting a thread
from the + button added it beside the thread already on screen, and Ash asked for "a new thread in
the same folder, not the double window thing", and for the grid to go entirely. The sidebar
already does the work tiles did: every thread is one click away, with its state on its row.

## Decision

- The main area shows one thread at a time. Clicking a sidebar row shows that thread; Ctrl+click
  does the same. A new thread or terminal takes the main area. There are no layouts, gutters,
  swaps or maximize, and no "Open beside".
- A file or a diff opens in a card over the window (`ui/src/workbench/card.rs`), on a dimmed
  backdrop like the palette's. Its header carries what the viewer pane's did: line counts, view the
  whole file, open in the editor, close. Esc or a click outside closes it and the keyboard goes
  back to what had it. Images Ctrl+clicked in a terminal open in their own lightbox
  (`viewer/lightbox.rs`), zoomable and movable.
- `proto::Grid` and `Space.grid` are gone. `Pane` stays: a dragged sidebar row carries one, and
  the viewer shows `Pane::File` and `Pane::Diff`. A state file that still has `grid` loads; serde
  skips the field.
- `ui/src/panes/` is now `ui/src/workbench/`: the thread, the bar, the dock and the viewer card.
- A terminal keeps its header (mark, title, model tag, close back to the composer) in an 8px
  margin. A structured thread runs edge to edge with its name in the bar, as a lone structured
  pane already did.

## Consequences

- Two threads can't be watched side by side. The sidebar's live rows (state, doing line,
  subagents on hover) are how several agents are followed at once.
- Auto-settle leaves only the thread on screen alone, since nothing else is on screen.
- Threads off screen keep their sessions running, as before; `Root.views` still holds one view
  per thread.
