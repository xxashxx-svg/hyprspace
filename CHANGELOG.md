# Changelog

Release notes for HyprSpace. Written **at ship time** — when you ask to ship, the agent looks at what
changed since the last release and writes a few user-facing bullets; `deploy.ps1` records them here
and uses them as the release notes + the in-app "What's new" notification. No per-task bookkeeping.

## 0.8.0 — 2026-07-02

- **Usage dashboard** (Settings → Usage): at-a-glance overview strip (total tokens, sessions, hottest rate limit with a live reset countdown), quota alerts when a rolling window crosses 80%, an all-time **"By model"** token breakdown for Claude, and token-based daily activity sparklines for Claude and Codex.
- **Usage loads progressively** — each provider's card streams in as its scan finishes (with branded skeletons), so the big Claude transcript scan no longer holds up the rest of the panel.
- **Streaming-safe emails** — account emails in Providers, Usage, and the home footer are blurred until clicked, and the settings sidebar shows just your name. Screen shares don't leak your accounts anymore.
- **Seamless terminal block art** — GPU (WebGL) rendering is now the default, so the Claude logo, progress bars, and box-drawing render as solid shapes at any line height. HyprSpace manages GPU per visible space (no context limits with many panes), and the renderer + line-height settings now actually persist across restarts (bug fix).
- **Editor: full screen & close** — new buttons in the editor header expand it over the whole workspace (Esc to exit) or close the file, with an unsaved-changes prompt.
- **"Don't ask me again"** on the close-running-pane confirmation — restore hidden dialogs anytime in Settings → Workspace.

## 0.7.1 — 2026-07-01

- Grok and OpenCode now appear in every launch surface — the titlebar "New" menu, the right-click "Open … here" menu, and the home-chat agent spawner. They were reachable but missing from a few of these menus in 0.7.0.

## 0.7.0 — 2026-07-01

- Added Grok (xAI's Grok Build CLI) as a first-class provider — launch it in terminal panes and the multi-agent launcher, run it in Loops, and see it in Providers & Usage. Uses your own `grok` login or XAI_API_KEY.
- New Usage panel in Settings: per-provider tokens, sessions, and rate limits with a token in/out/cache breakdown and a cleaner activity view — all read from each tool's local files, nothing leaves your machine.

## 0.6.2 — 2026-07-01

- macOS: fixed Claude, Gemini, Codex, and OpenCode showing as "not installed or not on PATH" — HyprSpace now adopts your real login-shell PATH, so provider detection, the home chat, and Loops all find the CLIs.
- Terminal: smoother resizing for Claude panes — throttled live reflow plus a full alt-screen repaint, so dragging no longer leaves smeared or duplicated rows.

## 0.6.1 — 2026-07-01

- **Sharper terminal text** by default — the crisp DOM/ClearType renderer, a cleaner default font (JetBrains Mono), and a solid block cursor.
- New **Line height** dial (Settings → Terminal) to tune row spacing live.
- Optional **GPU rendering** toggle (Settings → Terminal) — WebGL draws block art (logos, progress bars) seamlessly, at a slight cost to text sharpness. Off by default.
- More breathing room — extra horizontal padding so terminal text isn't flush against the pane edges.
- **Smoother under heavy output** — moved the scrollback search-indexer off the hot render path so firehose output doesn't drop frames.

## 0.6.0 — 2026-07-01

- **Actions** — the old Startup tab is now Actions: project-scoped commands you run on demand from the top-bar Actions menu, the command palette, or a keybinding. Auto-run when a project opens or a worktree is created is now opt-in per action (no more surprise re-runs).
- **Embedded preview** — give an action a Preview URL and it opens a docked in-app browser, so you can watch your dev server come up without leaving HyprSpace.
- **Layout picker** — choose how panes tile from a new titlebar button.
- **OpenCode** is now available in Settings → Providers.
- **Live git status** — Push / Commit / Create PR now show progress right on the button (Pushing… → Pushed) instead of finishing silently.
- **Fixes** — the sidebar project/space close (✕) button no longer jumps on hover; terminal block-art (the Claude logo, progress bars) renders cleanly without gaps.

## 0.5.15 — 2026-06-30

- Loops: new "Claude (subscription)" backend — run loops on your logged-in Claude, no API key needed.
- Loops: new "Interactive terminal" mode — a real Claude /goal session you can answer in, with a notification when it needs you; the Runs tab now shows the agent's actual responses.
- OpenCode added as a provider — launch opencode panes, use it as a headless Loops backend, and see it in Settings → Providers.
- Pane layout picker — a titlebar button to rearrange panes (columns, rows, 2-top-1-bottom, 1-left, ...), remembered per space.
- Create PR and Initialize repository are now full editable dialogs (title/body/base/draft; repo name, branch, .gitignore, README, optional GitHub create + push).
- Loops Manage redesigned into collapsible, sectioned cards; calmer status badges + running-loop card; settings lock while a loop runs.
- Sidebar tidy-up: loops moved into the Loops page (count on the nav); footer is just Settings.

## 0.5.14 — 2026-06-29

- "Claude (hooks)" loops now wait until Claude has finished booting before sending the prompt (the old fixed delay was too short on heavier Claude setups, which left the loop stuck on iteration 1), and the loop log now shows Claude's live terminal output so you can see what it's doing.

## 0.5.13 — 2026-06-28

- The "Claude (hooks)" Loops backend now works end to end: it drives a real interactive Claude session on your subscription (no API key) and loops until your until-check passes, the sentinel appears, or it hits max iterations. (It was hanging on startup before — the headless terminal now answers Claude's TUI queries and skips the trust/permission prompts.)
- Fixed: stopping a Claude-hooks loop could freeze the whole app ("Not Responding") — PTY teardown now happens off the UI thread, so Stop is instant.

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
