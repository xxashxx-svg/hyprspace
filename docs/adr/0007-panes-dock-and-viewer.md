# ADR 0007: A tiled grid per space, one viewer pane, and folder requests on the channel

- Status: Accepted; the grid and the viewer pane superseded by ADR 0015, the read-only viewer
  by ADR 0016
- Date: 2026-10-02

## Context

Phase 6a brings back four things the Tauri app has: several sessions on screen at once, the
right dock (files and git), file viewing, and opening a space's folder in an editor or the
file manager. The Tauri app tiles every session of a space as a pane, with layout presets,
draggable boundaries, drag-to-swap and double-click to maximize (`src/lib/grid.ts`,
`src/components/PaneGrid.tsx`). zeron shows one session at a time with a row of tabs on top, as
a viewport over its sidebar list.

The GPUI app also differs from the Tauri app in one way that matters: a space keeps every thread
it ever had in the sidebar, while a Tauri space only held its live panes. Tiling every thread
would put dozens of panes on screen.

## Decision

**Tiles, not tabs.** Running several agents side by side is what HyprSpace is for, and tabs
hide all but one. Each space has a `Grid` in its saved state (`proto/src/grid.rs`): the panes on
screen in layout order, the focused one, the maximized one, the preset picked per pane count,
and dragged track sizes per layout. The presets and the balanced tiling for one and seven-plus
panes are grid.ts's, as data in `ui/src/panes/layout.rs`. Panes sit on fractions of the frame,
so a dragged boundary needs no measuring, and a boundary a pane spans across stays fixed, as in
the Tauri app.

**The grid is a view over the threads, not the threads.** Clicking a sidebar row puts that
thread in the focused pane's place; ctrl+click (cmd on macOS) opens it beside the others; a new
thread joins as a new pane. Closing a pane takes it off screen and leaves the thread running
and in the sidebar, the same as opening another thread did before panes. A thread that is
removed or archived drops out of the grid when it is drawn, so nothing has to tidy saved grids.
When the last thread pane closes, the space shows its composer.

**One viewer pane per space.** A ctrl+clicked path, a file from the dock, or a changed file's
diff shows in the space's viewer pane, replacing what it showed, instead of piling up panes.
The viewer is read only: tree-sitter grammars (MIT, `crates/syntax`, adapted from zeron) color
it with the theme's terminal palette, so every theme and side keeps its own colors without new
tokens. Its pane header carries "open in editor", which still goes through `Command::OpenFile`.

**Folder requests ride one command and one event.** `Command::Folder(FolderCommand)` and
`Event::Folder(FolderEvent)` carry directory listings, file reads, git status, stage, commit,
push, diffs, the installed openers, and open-in. Each answer names the path it is about, the
way ADR 0002 planned. The engine runs them on the blocking pool behind one git lock, because
two quick ticks would otherwise race for git's `index.lock`. Change paths are relative to the
repo root, so stage and diff run there even when the dock follows a subfolder.

**The dock follows the focused thread's folder**, else the space's, and polls git every four
seconds while it is out, as the Tauri app did. The bar above the panes holds what the Tauri
titlebar held for a space: new thread, the layout picker, the Open split button, and the dock
toggle.

**Launches can be logged instead of run.** With `HYPRSPACE_OPEN_LOG` set, every editor,
Explorer or Finder launch appends its command line to that file. Checking the running app never
opens anyone's editor.

## Why not

- **zeron's tabs:** one session at a time is a step back from what users run today.
- **A pane per thread, as the Tauri app did:** with the GPUI app's thread history that is dozens
  of tiny panes.
- **Tab groups inside a slot** (the Tauri app stacked files as tabs in a pane): one viewer pane
  covers the same need with no second level of tabs.
- **CSS-grid layout in GPUI:** its grid takes equal tracks only, and dragged sizes are weights.
- **syntect:** pure Rust, but it ships a large syntax dump and is slower to load; tree-sitter is
  what zeron uses and its grammars are MIT.
- **Editing in the viewer:** the user's editor does that well and is one click away.

## Consequences

- A boundary crossed by a spanning pane can't be dragged, so "1 left, 2 right" resizes columns
  only. The Tauri app had the same limit.
- Sessions of threads restored into a grid start when their space is first shown, not at launch.
- The sidebar has no "Open beside" or "Open in" entries yet; those belong to its own code.
