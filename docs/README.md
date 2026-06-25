# HyprSpace docs

Start with **[../CLAUDE.md](../CLAUDE.md)** — the canonical project guide (overview, constraints,
repo map, quick reference). Everything here is the deeper material it links to.

| Doc | What it covers |
|---|---|
| [../CLAUDE.md](../CLAUDE.md) | **Read first.** What the app is, critical constraints, repo map, architecture overview, dev/build/deploy quick ref. |
| [ARCHITECTURE.md](./ARCHITECTURE.md) | How the tricky subsystems work: the persistent chat engine, the orchestrator, Loops (scheduled/looping agents) + the headless agent runner, startup services, PTY lifecycle, stores, persistence, auth. |
| [DEPLOY.md](./DEPLOY.md) | Release runbook — bump, build, sign, publish, macOS CI. Written to be followed **without Claude**. |
| [VERSIONING.md](./VERSIONING.md) | When to bump major/minor/patch, mapped to `deploy.ps1`. |
| [ENTITLEMENT.md](./ENTITLEMENT.md) | Subscription gating — how to flip free → paid (server-side, no app update). |
| [audit/README.md](./audit/README.md) | Security + bug audit (findings, severity, what's fixed vs. open). |
| [../BUILD-MAC.md](../BUILD-MAC.md) | Building the macOS `.dmg` locally. |

## Conventions for keeping docs current
- `CLAUDE.md` is the single source of truth for constraints/conventions. If you change a rule,
  update it there.
- `GEMINI.md` mirrors the essentials for non-Claude agents and points back here.
- When a feature changes how a subsystem works, update `ARCHITECTURE.md`.
- When the release process changes, update `DEPLOY.md`.
