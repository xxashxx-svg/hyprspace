# ADR 0003: Copy the Tauri app's Rust into the engine, never move it

- Status: Accepted
- Date: 2026-10-02

## Context

The engine needs the hard-won Rust in `src-tauri`: the PTY manager, the Claude hook listener, git
and worktrees, provider checks, local and live usage, skills, the resume list, the state store,
and the startup PATH fixes. The Tauri app is still the shipping product until the GPUI app reaches
parity, and it must keep building (`npm run tauri build`).

## Decision

Copy each file into `crates/engine`, adapt the copy, and leave `src-tauri` untouched. The Tauri
app is never tidied, only deleted at the end of phase 6.

While copying:

- Tauri types go: commands become plain functions, `Channel` becomes a callback or an engine
  event, `spawn_blocking` wrappers are left to the caller. No new crate depends on `tauri`.
- Comments that record why something works the way it does come along, with em dashes replaced.
- Paths and env lookups that made code untestable take a `home` argument internally, so tests run
  against a temp folder instead of the developer's real `~/.claude`.
- Left behind: licensing, the paywall and Supabase auth (unused), `git_root` (dead), the launcher
  remnants, the mobile bridge tap and replay buffer, the xterm pause gate, the headless terminal
  query answerer (the UI's emulator answers queries), and `fs.rs` (the dock's file tree, which
  comes with the dock).

Changes in behaviour, each small and on purpose:

- The live usage poller enforces the 180s Claude floor itself (60s for Codex). In the Tauri app
  only the React meter's interval kept to it.
- A git failure with an empty stderr reports stdout instead. `git commit` with nothing staged
  writes its reason to stdout, so the Tauri app showed an empty error there.
- The GPUI app keeps its state in `~/.hyprspace/native`, not `~/.hyprspace/v2`. The two apps run
  side by side during the rewrite with different schemas, so sharing a folder would let one
  clobber the other.

## Consequences

- A bug fixed in one copy is not fixed in the other. The Tauri app only gets bug fixes, so the
  rule is: fix it in `src-tauri` if users hit it now, and check whether the engine copy needs the
  same fix.
- Two copies of the same code live in the repo until phase 6 deletes `src-tauri`.
