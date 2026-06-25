# HyprSpace

A multi-terminal AI workspace — tile many Claude Code / Gemini / Codex / shell sessions across
**projects** and **open spaces**, with per-pane resume, drag-to-swap, a command palette, a git
review dock, themes, scheduled & looping agents (**Loops**), a home-screen AI chat that can operate
the app, and live auto-update. Built with **Tauri 2 + React**.

## Develop

```bash
npm install
npm run tauri dev
```

## Ship a release (and push an auto-update)

```powershell
.\deploy.ps1 patch "what changed"
```

Bumps the version, builds + signs the NSIS installer, publishes a GitHub release to the public
**hyprspace-releases** repo with a `latest.json` manifest, and triggers the macOS CI build.
Installed apps check that manifest on launch and update themselves. Signing keys live in
`~/.hyprspace-signing` (never committed). Use `none` instead of `patch` to publish the current
version as-is.

Pick the bump level (`patch`/`minor`/`major`) by [docs/VERSIONING.md](./docs/VERSIONING.md). Full
release steps — including how to do it **without** Claude — are in [docs/DEPLOY.md](./docs/DEPLOY.md).

## Docs

- **[CLAUDE.md](./CLAUDE.md)** — the project guide (read first): what it is, constraints, repo map,
  architecture overview, quick reference.
- **[docs/](./docs/README.md)** — architecture deep-dive, deploy runbook, versioning, security audit.
- **[BUILD-MAC.md](./BUILD-MAC.md)** — building the macOS app locally.
