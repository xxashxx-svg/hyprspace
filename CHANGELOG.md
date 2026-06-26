# Changelog

Release notes for HyprSpace. Written **at ship time** — when you ask to ship, the agent looks at what
changed since the last release and writes a few user-facing bullets; `deploy.ps1` records them here
and uses them as the release notes + the in-app "What's new" notification. No per-task bookkeeping.

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
