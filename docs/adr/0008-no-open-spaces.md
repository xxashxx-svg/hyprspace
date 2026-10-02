# 0008: No open spaces

Decided 2026-10-02 on the `rewrite-ui` branch, merged into `rewrite` the same day.

The Tauri app had two kinds of space: a project (one folder) and an open space (no folder, each
pane in its own). The GPUI app keeps only the first. "New thread" asks for a folder, reuses that
folder's space or adds one, then shows the composer there.

**Why.** Every thread runs in a folder anyway, and `claude --resume` only works in the folder a
conversation started in. An open space was a second place to put a thread that the folder already
answered, and it made the dock, the Open menu and resume handle a space with no folder. With one
kind, each of those can assume a folder.

**Cost.** Someone who grouped unrelated threads in one open space now sees them under their own
folders. proto's `Space` still allows `cwd: None`, so state saved before this change still loads.
