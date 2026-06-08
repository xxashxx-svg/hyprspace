# HyprSpace

A multi-terminal AI workspace — tile many Claude Code / shell sessions across **projects** and **open spaces**, with per-pane resume, drag-to-swap, themes, and live auto-update. Built with Tauri 2 + React.

## Develop

```bash
npm install
npm run tauri dev
```

## Ship a release (and push an auto-update)

```powershell
.\deploy.ps1 patch "what changed"
```

This bumps the version, builds + signs the NSIS installer, and publishes a GitHub release to the public **hyprspace-releases** repo with a `latest.json` manifest. Installed apps check that manifest on launch and update themselves.

Signing keys live in `~/.hyprspace-signing` (never committed). Use `none` instead of `patch` to publish the current version without bumping.
