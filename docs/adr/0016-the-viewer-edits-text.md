# ADR 0016: The viewer edits text files

- Status: Accepted
- Date: 2026-10-07
- Supersedes: "the viewer is read only" in ADR 0007

## Context

ADR 0007 kept the viewer read only and put "open in editor" in its header. Ash asked to edit
Markdown and other text files in place, "like VS Code". Zed's `editor` crate is GPL-3.0 and stays
out of this MIT repo (docs/REWRITE.md). zeron edits files through its fork of longbridge's
`gpui-component` (Apache-2.0), but that crate builds against a published GPUI snapshot
(`gpui-pre`), not the Zed commit we pin, so it can't drop in without two copies of GPUI. Ash chose
a focused editor of our own over porting it.

## Decision

- A text file opens in `viewer/edit/`: a buffer of one string with its line starts
  (`buffer.rs`), an element that shapes only the lines on screen (`element.rs`), and the text
  box's IME bridge over the whole file (`handler.rs`). It does typing and IME, selection by keys
  and mouse, cut, copy and paste, undo and redo, Enter keeping the indent, Tab and Shift+Tab over
  lines, and Ctrl+S. Syntax colors are worked out again 150 ms after typing rests. Find and
  replace, several cursors and completion are left out for now.
- Saving is `FolderCommand::WriteFile` with the text the editor read as `expect`. The engine
  writes only while the file still holds it, and refuses a file that isn't UTF-8, since the
  viewer reads files lossily. A changed file answers `SaveError::Changed`, and the editor offers
  Reload or Overwrite.
- An open file nobody has edited here is read again every 2 seconds and follows the disk, so an
  agent's edits show up. With unsaved edits, a change on disk raises the same notice instead.
- Closing the card with unsaved edits asks Save, Don't save or Cancel. A closed card drops its
  editor, so the next look reads the file fresh.
- The file's line breaks are kept: a typed or pasted break takes the file's style.

## Consequences

- Two writers can still race between our read and our write, a window of milliseconds; the
  check catches anything that landed before the save.
- Every open file costs one read every 2 seconds while its card is up.
- Files over 2 MB or binary still don't open, as before.
