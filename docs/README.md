# HyprSpace docs

Start with **[../CLAUDE.md](../CLAUDE.md)**, the canonical project guide (overview, constraints,
repo map, quick reference). Everything here is the deeper material it links to.

| Doc | What it covers |
|---|---|
| [../CLAUDE.md](../CLAUDE.md) | **Read first.** What the app is, critical constraints, repo map, architecture overview, run, build and check. |
| [ARCHITECTURE.md](./ARCHITECTURE.md) | How the tricky subsystems work: the channel, structured sessions, terminal sessions and hooks, PTY lifecycle, panes, persistence and journals, usage, updates. |
| [CONTEXT.md](./CONTEXT.md) | The domain words (space, thread, session, run, harness...), defined once. |
| [adr/](./adr/) | One short record per decision someone would later question, with the reason. |
| [REWRITE.md](./REWRITE.md) | History: the October 2026 rebuild from Tauri + React to GPUI, its phases, billing facts, parity checklist, and what was deleted. |
| [../mobile/README.md](../mobile/README.md) | The Android companion app. It paired with the Tauri app's bridge and is redone later. |
| [VERSIONING.md](./VERSIONING.md) | When to bump major/minor/patch. |
| [BUILD-MAC.md](./BUILD-MAC.md) | Building the macOS app and dmg locally. |
| [CHANGELOG.md](./CHANGELOG.md) | Release notes per version, also bundled into the app's "What's new". |

## Conventions for keeping docs current
- `CLAUDE.md` is the single source of truth for constraints/conventions. If you change a rule,
  update it there.
- When a feature changes how a subsystem works, update `ARCHITECTURE.md`.
- A decision someone would later question gets a new file in `adr/`; a new domain word goes in
  `CONTEXT.md` first.
