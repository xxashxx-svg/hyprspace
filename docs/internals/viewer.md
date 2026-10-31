# Dock, viewer and editor

## Folder requests

Listings, reads, writes, git status, stage, commit, push, diffs, the installed openers and
open-in all ride `Command::Folder` / `Event::Folder`, each answer naming the path it's about.

- **One git lock.** The engine runs them on the blocking pool behind one lock, because two quick
  ticks would otherwise race for git's `index.lock`.
- Change paths are relative to the repo root, so stage and diff work when the dock follows a
  subfolder.
- **Git status follows the files** (`engine/src/watch.rs`). The UI names the folders it shows
  (`FolderCommand::Watch`) and the engine watches each one's repo, reading its status again once
  changes go quiet for 400 ms, and at most every 2 seconds while they don't. Writes under
  `.git/objects` and `.git/logs` don't count; an overflowed watcher counts as a change everywhere.
  The timed reads that remain (the dock every 30 seconds, the sidebar every minute and every 10
  minutes) only catch what a watcher misses, such as a worktree's branch, which lives in the main
  repo's `.git`.
- **Opening things outside the app** (`engine/src/open.rs`): VS Code or Cursor with
  `--goto file:line:col`, Explorer or Finder for folders. Only paths that exist reach a launcher.
  `HYPRSPACE_OPEN_LOG` appends each launch's command line to a file instead of running it, so
  checking the app never opens anyone's editor.

## The viewer card

A file or one file's diff opens in a card over the window (`ui/src/workbench/card.rs`), colored by
`crates/syntax` (tree-sitter, MIT grammars) with the theme's terminal palette, so every theme keeps
its own colors without new tokens. Esc or a click outside closes it and gives the keyboard back.
Reads are capped at 2 MB; past that, or for media, the file goes to the user's editor.

## Diffs

- **One renderer.** `ui/src/viewer/diff.rs` draws every diff: the card, an edit in the
  transcript, and the approval dock, which use smaller rows that wrap and an 8% wash instead of
  10%. Syntax colors come from a background pass that highlights each file's new side and old side
  as one piece, so a line is colored in context. The rows show plain until it lands. The
  transcript keeps parsed and colored edits in a cache keyed by their text, so a scrolled-back
  card doesn't parse or highlight again.
- **A run's changes come from its snapshots.** A run's changes card (`RunEvent::Changes`) holds
  the git trees taken as the run started and ended, the same ones Undo uses. View changes diffs
  those two trees (`FolderCommand::Changes`), so the card's counts and the diff always agree, and
  the diff never polls. The trees belong to no ref, so `git gc` can drop them after about two
  weeks; the viewer then says the snapshots are gone.
- **Wider scopes.** Uncommitted diffs HEAD against a fresh snapshot, so new files show and the
  repo's index is untouched. Branch diffs the merge base with the default branch (`origin/HEAD`'s
  name, else `main` or `master`, local first) against a fresh snapshot. On the default branch, or
  with none to find, Branch is greyed out. Both poll like the dock's diff.
- **Threads on other computers** get no changes card, because `Changes` stays out of the journal a
  phone or computer watches, so View changes is this computer only.

## The editor

A text file in the card is editable (`ui/src/viewer/edit/`). It's our own, small on purpose: Zed's
`editor` is GPL, and zeron's editor comes from `gpui-component`, which builds against a published
GPUI snapshot rather than the Zed commit we pin.

- It does typing and IME, selection by keys and mouse, clipboard, undo and redo, Enter keeping the
  indent, Tab and Shift+Tab over lines, and Ctrl+S. Find and replace, several cursors and
  completion are left out for now. Syntax colors are worked out again 150 ms after typing rests.
- **Saving only over what it read.** `FolderCommand::WriteFile` carries the text the editor read
  as `expect`, and the engine writes only while the file still holds it. A changed file answers
  `SaveError::Changed` and the editor offers Reload or Overwrite. Two writers can still race in the
  milliseconds between our read and our write. A file that isn't UTF-8 is refused, since the
  viewer reads files lossily.
- **Following the disk.** A file nobody has edited here is read again every 2 seconds, so an
  agent's edits show up. With unsaved edits, a change on disk raises the same notice instead.
- Closing with unsaved edits asks Save, Don't save or Cancel. A closed card drops its editor, so
  the next look reads the file fresh.
- The file's line breaks are kept: a typed or pasted break takes the file's style.
