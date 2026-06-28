# Changelog

Release notes for HyprSpace. Written **at ship time** — when you ask to ship, the agent looks at what
changed since the last release and writes a few user-facing bullets; `deploy.ps1` records them here
and uses them as the release notes + the in-app "What's new" notification. No per-task bookkeeping.

## 0.5.12 — 2026-06-28

- New "Claude (hooks)" Loops backend: run a loop on your Claude subscription with no API key. It drives a real interactive Claude session via a Stop hook that keeps going until your until-check passes, the sentinel shows up, or it hits max iterations — plus an optional /goal mode that uses Claude's built-in goal loop.
- macOS: you can now drag the title bar to move the window and double-click it to maximize (fixes a Tauri overlay-titlebar quirk).

## 0.5.11 — 2026-06-28

- Right-click a project or open space in the sidebar for a context menu — rename, open its folder, copy its path, or remove it
- The sidebar no longer starts with a default "Home" project; it begins empty so you add your own projects and spaces
- Fixed: adding or editing an MCP server could wipe ~/.claude.json if the file was mid-write or invalid — it's now read safely and written atomically
- Fixed: restarting a background service no longer shows it as stopped while it's actually still running

## 0.5.10 — 2026-06-27

- Redesigned the theme picker in Settings — each theme is now a card with a live mini-preview of the app rendered in that accent, plus a short hue description
- Keyboard shortcut hints now use the proper Mac modifier keys (Command/Shift/Option) on macOS instead of Ctrl

## 0.5.9 — 2026-06-27

- The multi-agent launcher is now a full-page launchpad instead of a cramped dialog — more room to set your folder, grid size, and agent mix
- New live layout preview shows exactly how your terminals will tile, with each cell marked by the agent that will run there
- Agents now show their real brand logos and a one-line description, alongside the same quick-fill, presets, and permission controls

## 0.5.8 — 2026-06-27

- Refreshed terminal look: a softer T3-style color palette, a background that blends into the app, and crisp native text
- Ctrl+scroll now zooms terminals both in and out (it only zoomed one way before)
- Terminal panes stay evenly sized after maximizing and restoring
- Fixed a rare blank screen on launch where the app could hang while restoring your last session
- Agent panes are named after their folder now, with optional AI task-naming you can turn on in Settings

## 0.5.7 — 2026-06-27

- The launchpad now shows each tool's real logo (Claude, Gemini, Codex, WSL) instead of generic icons
- Smoother sidebar: the file tree opens cleanly without the jumpy pop, and collapse/expand is more fluid
- Loops: a live runs view, and your loops now appear in the sidebar with live status and progress

## 0.5.6 — 2026-06-27

- New launchpad: the empty-space view is now a launch hub with app-style icons for Claude, Gemini, Codex, WSL and Terminal, a 'launch several agents' shortcut, and one-click saved presets
- Cold-launching panes show a 'Starting…' spinner while the shell or agent boots, instead of a blank black screen

## 0.5.5 — 2026-06-26

- Maintenance release to verify the new in-app update experience (progress bar + clear Downloading/Installing/Restarting status)

## 0.5.4 — 2026-06-26

- A cleaner update flow: a progress bar and clear status (Downloading, Installing, Restarting) while updating, instead of plain text
- If an update fails, it now shows the error with a Retry button

## 0.5.3 — 2026-06-26

- Fixed a glitch where the focused terminal pane could lose its header or shift its content
- Better multi-pane layouts: panes now split into balanced rows (7 becomes 4 + 3) instead of an unusable grid
- Updates jump straight to the newest version in one step, even when you're several releases behind
- A 'What's new' note now appears in notifications after the app updates

## 0.5.2 — 2026-06-26
- New cube logo + app/installer icon, and website favicon
- Smooth animations across the sidebar, file tree, Loops, and presets
- Removed the bottom status bar
- Internal: split the styling and dev-cockpit code into focused modules

## 0.5.1 — 2026-06-26
- Square terminal panes (dropped the rounded corners)

## 0.5.0 — 2026-06-26
- Multi-agent launcher — fan out many agents in a folder at once, with saved presets
- Integrated code editor with a file tree (open, edit, save / autosave)
- Loops backends: run on an Anthropic API key (kept separate from your subscription), Codex, or Gemini
- Refined the UI toward a calmer, T3-style look
