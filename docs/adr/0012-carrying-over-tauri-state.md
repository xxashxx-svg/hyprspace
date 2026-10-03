# ADR 0012: Bringing the Tauri app's spaces and settings over once

- Status: Accepted
- Date: 2026-10-02

## Context

The Tauri app keeps its state in `~/.hyprspace/v2` (`workspaces.json`, `settings.json`,
`lastSeenVersion.json`, written by `src-tauri/src/persist.rs`). The GPUI app keeps its own in
`~/.hyprspace/native` with a different shape. Someone the Tauri updater moves to the GPUI app
must find their projects there.

## Decision

**On a first run only, read v2 and never write it.** When `native/state.json` is absent,
`engine/src/legacy.rs` reads the `v2` folder beside it and the result is saved at once, so it
happens one time and later changes to v2 (the Tauri app may still be installed elsewhere) don't
leak in. A file that is missing or broken brings over nothing from that file and the rest still
comes. The read is tested against a fixture (`engine/tests/fixtures/tauri-v2`) that also checks
the files are byte for byte unchanged.

**What comes over:**

- every project as a space, in the Tauri order, archived ones still archived, one per folder
  (a repeat of a folder, with or without a trailing slash, is skipped);
- open spaces are gone (ADR 0008), so each folder an open space's panes ran in becomes a space of
  its own unless one exists. Skipping them would drop folders people worked in;
- theme and light, dark or system; each agent's model and effort; the last agent used; its
  permission mode (Claude's `acceptEdits` and Codex's `auto` are Auto); the sidebar and dock
  widths; what the Open button opens; whether the intro was seen;
- `lastSeenVersion`, so What's new shows after the update (ADR 0011).

- each agent pane as a terminal thread in its folder's space (see the amendment below).

**What doesn't:** viewer tabs, unsent drafts and automation runs. Terminal look, fonts and the
settings the GPUI app has no place for stay behind.

## Amendment, 2026-10-03: panes come over as threads

This first said panes don't become threads, since the composer's resume list offers every saved
conversation. In practice someone switching apps lost their sidebar: the threads they had open,
by name, were gone. So each Claude, Codex or Gemini pane now becomes a terminal thread with its
title, model, effort and permission (read off its launch command). A Claude thread resumes the
conversation the pane was on: its `claudeSessionId`, else the id its command resumed, else its
own id, whichever has a transcript on disk first, and the transcript's last write dates the
thread. Codex panes never knew their conversation, so theirs start fresh.

Grids start empty, so nothing launches until a thread is opened. That keeps a Tauri app that is
still running from sharing a live conversation with this one.
