# HyprSpace docs

Start with **[../CLAUDE.md](../CLAUDE.md)** — the canonical project guide (overview, constraints,
repo map, quick reference). Everything here is the deeper material it links to.

| Doc | What it covers |
|---|---|
| [../CLAUDE.md](../CLAUDE.md) | **Read first.** What the app is, critical constraints, repo map, architecture overview, dev/build/deploy quick ref. |
| [../AGENTS.md](../AGENTS.md) | Working in parallel with other agents — where to work so you don't collide, shared hot-files, the claim board. |
| [ARCHITECTURE.md](./ARCHITECTURE.md) | How the tricky subsystems work: Loops + the headless agent runner, the multi-agent launcher, the integrated editor, startup services, PTY lifecycle, stores, persistence, auth. |
| [DEPLOY.md](./DEPLOY.md) | Release runbook — bump, build, sign, publish, macOS CI. Written to be followed **without Claude**. |
| [VERSIONING.md](./VERSIONING.md) | When to bump major/minor/patch, mapped to `deploy.ps1`. |
| [ENTITLEMENT.md](./ENTITLEMENT.md) | Subscription gating — how to flip free → paid (server-side, no app update). |
| [audit/README.md](./audit/README.md) | Security + bug audit (findings, severity, what's fixed vs. open). |
| [../BUILD-MAC.md](../BUILD-MAC.md) | Building the macOS `.dmg` locally. |

## Conventions for keeping docs current
- `CLAUDE.md` is the single source of truth for constraints/conventions. If you change a rule,
  update it there.
- `AGENTS.md` is the coordination guide for multiple agents — update it if the repo layout or the
  shared hot-files change.
- When a feature changes how a subsystem works, update `ARCHITECTURE.md`.
- When the release process changes, update `DEPLOY.md`.
