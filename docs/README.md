# HyprSpace docs

Start with **[../CLAUDE.md](../CLAUDE.md)** — the canonical project guide (overview, constraints,
repo map, quick reference). Everything here is the deeper material it links to.

| Doc | What it covers |
|---|---|
| [../CLAUDE.md](../CLAUDE.md) | **Read first.** What the app is, critical constraints, repo map, architecture overview, dev/build quick ref. |
| [ARCHITECTURE.md](./ARCHITECTURE.md) | How the tricky subsystems work: Loops + the headless agent runner, the multi-agent launcher, the integrated editor, startup services, PTY lifecycle, stores, persistence, auth. |
| [VERSIONING.md](./VERSIONING.md) | When to bump major/minor/patch. |
| [BUILD-MAC.md](./BUILD-MAC.md) | Building the macOS `.dmg` locally. |
| [CHANGELOG.md](./CHANGELOG.md) | Release notes per version. |

## Conventions for keeping docs current
- `CLAUDE.md` is the single source of truth for constraints/conventions. If you change a rule,
  update it there.
- When a feature changes how a subsystem works, update `ARCHITECTURE.md`.
