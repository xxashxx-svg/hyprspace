# Changelog

Release notes for HyprSpace. Written **at ship time** — when you ask to ship, the agent looks at what
changed since the last release and writes a few user-facing bullets; `deploy.ps1` records them here
and uses them as the release notes + the in-app "What's new" notification. No per-task bookkeeping.

## 0.11.16 — 2026-07-24

Fixed: opening an image (Ctrl/Cmd-click an [Image #N] marker) shows the image again instead of the terminal.

## 0.11.15 — 2026-07-24

macOS: the native window buttons (close / minimize / maximize) now appear in the top-left — they were being hidden on macOS 26.

## 0.11.14 — 2026-07-24

macOS now auto-updates automatically, like Windows — no more manual .dmg downloads. Also includes the macOS window close/traffic-light button fix.

## 0.11.13 — 2026-07-24

macOS: no more 'Couldn't reach the update server' toast on launch — the updater now treats a platform with no update channel as up-to-date, and only a manual 'Check now' surfaces a real error. Auto-checks stay silent on transient failures.

## 0.11.12 — 2026-07-24

Fixed: opening an image tab (or a new agent tab) in a single-pane slot no longer restarts the terminal when you switch back to it — the pane and its running agent stay alive instead of remounting.

## 0.11.11 — 2026-07-24

Ctrl+click Claude's [Image #N] markers in a terminal to reopen that pasted image in a viewer tab. macOS: fixed the top-left close / traffic-light buttons — clicking near the corner no longer starts a window drag instead of closing. Image paths in terminal output are also more reliably clickable now (wrapped paths + existence-checked).

## 0.11.10 — 2026-07-24

Paste images into any agent — copy an image and press Ctrl+V (or Alt+V) in a terminal; it drops in as a file the agent can read, now near-instant instead of taking seconds. New built-in image viewer: Ctrl+click an image path in output to open it as a tab (fit or 1:1). Settings now hides the New/Open/Commit/Actions bar for a cleaner screen.

## 0.11.9 — 2026-07-24

Cleaner sidebar toggle icon in the top-left: the left rail is solid while the sidebar is out and hollow once it's hidden, so the state reads at a glance.

## 0.11.8 — 2026-07-24

You can now fully hide the left sidebar: click the toggle next to the logo (top-left) to slide it away, and again to bring it back. Removed the old edge collapse handle and the wordmark for a cleaner top bar.

## 0.11.7 — 2026-07-15

Hide the review-dock toggle on the home screen — it did nothing there and only added clutter.

## 0.11.6 — 2026-07-14

- Sidebar Search is now a live filter: type to instantly narrow your projects, open spaces, and sessions by name. The Ctrl K chip still opens the full command palette.
- Files panel gained a "Find files" box — recursive filename search across the project; click a result to open it. Plus a cleaner, roomier tree that matches the app's look.

## 0.11.5 — 2026-07-14

- The sidebar is cleaner: projects expand to their sessions only — files moved fully into the dock's Files panel.
- The Files panel grew a real context menu: New file / New folder (inline, right in the tree), Copy path / Copy relative path, Rename, and Delete (with confirmation) — alongside the existing open-agent-here and Reveal options.

## 0.11.4 — 2026-07-14

- Sidebar now shows the provider logo on each agent session — Claude, Codex, Gemini, Grok, OpenCode — with its status as a small corner badge, like Orca.
- Sidebar polish: a clear rounded "selected" highlight, airier rows, more spacing between sections, and smoother fade-in hovers.

## 0.11.3 — 2026-07-14

- Fixed a major memory blowup: the app was starting every session in every space at once on launch, which could use many GB of RAM and freeze. It now only starts the space you actually open — launch is light and fast.
- Automations actually run now: fixed a bad launch command so an automation opens a real Claude session on your goal and finishes cleanly when the task is done.
- Editor restyled to a VS Code (Dark+) look — familiar syntax colors and selection.
- Smoother, cleaner sidebar: brighter headers, fade-in hover states, tidier badges.
- Terminal renders wide and ZWJ emoji at the correct width (no more mis-aligned lines).
- Refined, consistent scrollbars across the whole app.

## 0.11.2 — 2026-07-08

- Fixed flickering when dragging to reorder projects and open spaces in the sidebar. Drag feedback is now applied directly to the DOM, so moving a row no longer re-renders the whole rail.

## 0.11.1 — 2026-07-04

- Redesigned the terminal color-theme picker: each card is now a miniature terminal preview with centered theme names

## 0.11.0 — 2026-07-04

- Terminal color themes: pick from 9 schemes (Claude Code, One Dark, Dracula, Catppuccin Mocha, Tokyo Night, Nord, Gruvbox, Solarized) in Settings → Terminal — applies live to every pane
- Bundled JetBrainsMono Nerd Font: terminals now render identically on every machine, with proper powerline/status-line glyphs (no more tofu boxes)
- Terminal polish: theme colors now fill the whole pane (no edge bleed), scrollbar hides until you hover, roomier text padding

## 0.10.0 — 2026-07-02

- **First-run onboarding**: new installs get a short setup wizard that checks which agent CLIs are installed (with copyable install commands), sets the Claude permission mode and theme, and opens your first workspace. Replay it anytime from Settings, About.
- **New app icon**: the isometric cube on a dark tile, now consistent across the app, taskbar, shortcuts, and the website.
- **Automations reliability batch**: stopping an interactive automation no longer double-records history or fires a bogus "finished" notification; monthly+ cron schedules fire at the right time instead of immediately; scheduled automations now show "Scheduled, next run ..." instead of looking stuck; Codex sessions keep their sandbox and model settings across iterations; deleting a running automation cleans up properly; diff summaries only count what the run actually changed.
- **The editor never silently loses work**: unsaved changes are flushed to disk when you switch files, tabs, or views, unless you explicitly chose Discard.
- **Confirm dialogs are keyboard-safe**: Enter no longer triggers the destructive action while Cancel is focused, and plain terminals are no longer mistaken for running services when closing.
- **Usage numbers now match Claude's own /usage**: token headlines count input + output (cache re-reads shown separately), and the By-model list ranks by real usage.
- **Under the hood**: hardened process cleanup, safer OAuth callback reads, rename-safe git status parsing, size caps on large file scans, and pipe writes moved off the UI thread so a wedged agent can't freeze the window.
- The home-screen chat is retired, theme cards are simpler (default theme is now called Midnight), and copy has been cleaned up across Settings.

## 0.9.0 — 2026-07-02

- **Loops are now Automations** — same engine, clearer name, everywhere in the app.
- **Real cron schedules** — give an automation a full 5-field cron expression (`*/30 9-18 * * 1-5`) with a live "next run" preview, alongside the existing interval and daily options.
- **Run history** — every finished run is saved across restarts: status, iterations, duration, cost, and a "Past runs" list on the Runs tab with one-click access to each run's worktree.
- **Outcome at a glance** — when a run ends you get a notification with the result and a diff summary ("4 files · +120 −80"), shown in the status bar and history too.
- **Smarter failure handling** — failed iterations retry with backoff (5s/10s/20s); three failures in a row stops the run and alerts you instead of burning quota.
- **Codex upgrades** — automations on Codex now show a live structured transcript (commands, reasoning, output) and count tokens, so token budgets work for Codex as well as Claude.
- **Interactive mode for Codex & Gemini** — "In a terminal" automations aren't Claude-only anymore.
- **Redesigned automation editor** — backend picker with brand logos, segmented run/schedule controls, proper toggle switches, and grouped sections that match the rest of the app.

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
