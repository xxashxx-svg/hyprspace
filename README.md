# HyprSpace

A multi-terminal AI workspace — tile many Claude Code / Gemini / Codex / shell sessions side by side
and drive them from one window. Built with **Tauri 2 + React**.

![HyprSpace workspace](./website/src/assets/shots/workspace.png)

HyprSpace organises work into **projects** (a folder) and **open spaces** (a scratch space whose
panes can each sit in a different folder). Each pane is a real PTY running a real agent CLI, so
anything you'd do in a terminal still works.

- **Multi-agent launcher** — fan out N agents across a folder in one shot, tiled automatically.
- **Per-pane resume**, drag-to-swap panes, a command palette.
- **Loops** — scheduled, interval, until-done or manual agents with mandatory stop limits and
  optional git-worktree isolation.
- **Git review dock** + an integrated CodeMirror editor for reviewing and fixing what the agents did.
- **Startup services** per folder, themes, and auto-update.

It runs agents on *your* CLIs and *your* logins — HyprSpace spawns the `claude` / `gemini` / `codex`
binary you already have installed and authenticated. It never handles your subscription credentials.

## Prerequisites

- **Rust**, stable toolchain, via [rustup](https://rustup.rs).
- **Node 18+** (20 or 22 recommended) and npm.
- **Windows:** the MSVC C++ build tools ("Desktop development with C++" in the Visual Studio Build
  Tools) plus the **WebView2** runtime — shipped with Windows 11 and current Windows 10, otherwise
  grab the evergreen installer from Microsoft.
- **macOS:** `xcode-select --install`. See [BUILD-MAC.md](./BUILD-MAC.md) for producing a `.dmg`.
- **Linux:** the standard Tauri deps (`webkit2gtk-4.1`, `libsoup-3.0`, `librsvg2`, `build-essential`).
  Linux mostly works but isn't shipped.

To launch agents you'll also want at least one agent CLI installed and logged in — `claude`,
`gemini`, or `codex`. Panes with a plain shell work without any of them.

## Quick start

```bash
git clone https://github.com/xxashxx-svg/hyprspace.git
cd hyprspace
npm install
cp .env.example .env      # optional, see below
npm run tauri dev
```

First run compiles the Rust side, so give it a few minutes. After that, TS changes hot-reload and
Rust changes trigger a recompile + relaunch.

`.env` holds `VITE_SUPABASE_URL` and `VITE_SUPABASE_ANON_KEY`. Supabase is used for **sign-in only**,
and auth fails open in dev — **the app runs fine without them**, you just won't have an account.
Fill them in only if you're wiring up your own Supabase project.

Production build:

```bash
npm run tauri build       # Windows NSIS installer by default
```

## Architecture

The short version: panes are PTYs managed by Rust (`src-tauri/src/pty.rs`); the launch command is
typed into the shell as keystrokes rather than passed as argv; state lives in Zustand stores and is
persisted by `src-tauri/src/persist.rs`; the frontend never calls Tauri `invoke()` directly, it goes
through `src/api/index.ts`.

- **[CLAUDE.md](./CLAUDE.md)** — the project guide: repo map, constraints, architecture overview.
  Read this before writing code (it's written for humans and AI agents alike).
- **[docs/](./docs/README.md)** — deep dives on the subsystems, versioning, and the security audit.

## Contributing

Issues and PRs welcome. [CONTRIBUTING.md](./CONTRIBUTING.md) covers the dev setup, the repo layout,
and the style rules that matter (vanilla CSS + design tokens, no Tailwind, no `React.StrictMode`).
Please also read the [Code of Conduct](./CODE_OF_CONDUCT.md).

Security issues go through a private advisory, not an issue — see [SECURITY.md](./SECURITY.md).

## License

MIT — see [LICENSE](./LICENSE).

---

### Releasing (maintainers)

Releases are cut with `.\deploy.ps1 <none|patch|minor|major> "what changed"`, which bumps the
version files, builds and signs the installer, and publishes it with an update manifest. It needs
the project's signing keys, so it's maintainer-only. Runbook:
[docs/DEPLOY.md](./docs/DEPLOY.md); bump levels: [docs/VERSIONING.md](./docs/VERSIONING.md).
