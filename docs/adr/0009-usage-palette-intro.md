# ADR 0009: Usage readings in one entity, a palette over the root, an intro without live demos

- Status: Accepted
- Date: 2026-10-02

## Context

Phase 6b brings back the usage meter, Settings' Usage and Skills views, the command palette and
the intro. CLAUDE.md rule 1 and its Usage section set how the live limits may be read: the
token the CLI already stores, the usage endpoint only, Claude no more than every 180 seconds.

## Decision

**The engine owns the floor, the UI owns the cadence.** `engine/src/usage/live.rs` answers any
request inside the floor (180s Claude, 60s Codex) from its last reading and backs off on 429 and
5xx, so no view can ask faster by mistake. One `ui::usage::Limits` entity asks on a 30-second
tick when each is due and holds every reading, so the ring and Settings never disagree and
opening Settings fetches nothing. Codex is asked every 60s as CLAUDE.md's Usage section says;
the Tauri meter asked every 187s. Codex's session files are read only while its live reading has
no windows, instead of whenever a Codex pane was open.

**Status-line limits cross the channel as parsed reports.** The hook listener already tees
Claude's status line; `usage/status.rs` turns its `rate_limits` into the same `LiveBar` the
endpoints produce, sent as `UsageEvent::StatusLine` per session. The UI folds them the way the
Tauri app's `summarize` did: freshest report with windows wins, old ones are marked stale.

**Fixtures instead of the endpoints for checks.** `HYPRSPACE_USAGE_FIXTURES` points the live
readers at files (with a `fixtureStatus` to fake a 401 or 429), so the running app can be checked
without spending the request bucket Claude Code shares.

**The palette floats over the root.** It is an entity the root opens and closes, drawn as a
deferred layer, bound to Ctrl+K in any context but `Terminal` (where Ctrl+K is kill-line) and to
Ctrl+Shift+P everywhere. Its arrows are bound in `Palette > TextInput` after the text box's own
keys, so they win inside it. The root builds the items fresh on each open and runs the one picked.

**The intro draws sketches, not working demos.** The Tauri intro ran small interactive copies of
the sidebar, composer, panes, palette, terminal, git tab and meter. Here each topic is a still
drawing in the theme's tokens. It shows once when the saved state has no spaces; anyone with
spaces gets `intro_seen` set without seeing it. Sign-in and licensing steps are gone with the
Supabase chain.

**Snippets are dropped from Skills.** They were text dragged into a terminal from the Tauri
dock. The GPUI app has no snippet dock, so a list of them would have no use.

## Consequences

- Settings' render re-focuses its own container whenever it is not focused, which steals focus
  from any text box inside it and from a palette opened over it. The fix is in the settings code:
  focus the screen once when it opens (`Root::open_settings`) and drop the per-render grab.
- The palette's Settings entry opens the last tab shown; per-tab entries need a way to pick the
  tab from outside the settings module.
