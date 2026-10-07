# Dock, viewer and editor

## Folder requests

Listings, reads, writes, git status, stage, commit, push, diffs, the installed openers and
open-in all ride `Command::Folder` / `Event::Folder`, each answer naming the path it's about.

- **One git lock.** The engine runs them on the blocking pool behind one lock, because two quick
  ticks would otherwise race for git's `index.lock`.
- Change paths are relative to the repo root, so stage and diff work when the dock follows a
  subfolder. The dock polls git every four seconds while it's out.
- **Opening things outside the app** (`engine/src/open.rs`): VS Code or Cursor with
  `--goto file:line:col`, Explorer or Finder for folders. Only paths that exist reach a launcher.
  `HYPRSPACE_OPEN_LOG` appends each launch's command line to a file instead of running it, so
  checking the app never opens anyone's editor.

## The viewer card

A file or one file's diff opens in a card over the window (`ui/src/workbench/card.rs`), colored by
`crates/syntax` (tree-sitter, MIT grammars) with the theme's terminal palette, so every theme keeps
its own colors without new tokens. Esc or a click outside closes it and gives the keyboard back.
Reads are capped at 2 MB; past that, or for media, the file goes to the user's editor.

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
