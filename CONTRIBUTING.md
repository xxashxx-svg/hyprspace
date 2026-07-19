# Contributing to HyprSpace

Thanks for taking a look. The frontend is TypeScript on Vite, the backend is Rust on Tauri 2.

## Getting set up

Prerequisites and the clone-to-running steps are in the
[README](./README.md#prerequisites). Once `npm run tauri dev` works, you're ready.

To actually launch agents you need whichever CLI you want to use already installed and logged in
(`claude`, `gemini`, `codex`). HyprSpace spawns them, it never authenticates on their behalf.

## Repo layout

[CLAUDE.md](./CLAUDE.md) has the full repo map and an architecture overview, and it's the canonical
reference. Read it first. The one-paragraph version:

- `src/` is the React frontend. `components/` for UI, `stores/` for Zustand state, `api/index.ts` for
  the typed bridge over Tauri `invoke()`, `styles/` for per-area CSS.
- `src-tauri/src/` is the Rust side. `lib.rs` registers every command, `pty.rs` is the terminal
  backend, `devtools/` is the git/fs/project command surface, `persist.rs` is the state store.
- `docs/` is the deeper material, starting at [docs/README.md](./docs/README.md).
- `website/` is the marketing site, a separate Vite app with its own
  [README](./website/README.md).

## Code style

The full list of hard constraints lives in [CLAUDE.md](./CLAUDE.md). The ones that bite contributors
most often:

- **Vanilla CSS and design tokens only.** Use the variables in `src/styles/tokens.css`. No Tailwind
  and no CSS-in-JS in the app. (The `website/` sub-project does use Tailwind. That's deliberate and
  separate.)
- **No `React.StrictMode`.** It's off in `main.tsx` on purpose, because double-mounting corrupts
  xterm.js lifecycles. Don't re-add it.
- **Don't churn `TerminalPane`.** Avoid patterns that frequently unmount and remount it, and always
  dispose xterm instances and addons on cleanup. Orphaned ConPTY hosts burn CPU.
- **Components call `src/api/index.ts`**, never `invoke()` directly. Filesystem-heavy Tauri commands
  should be `async fn` plus `spawn_blocking`, since sync commands run on the UI thread.
- **Keep comments minimal and casual.** Explain tricky logic only, lowercase, short. Match the
  surrounding code rather than your own preferred style.
- **Never hand-edit version numbers.** `deploy.ps1` owns the `version` field in
  `src-tauri/tauri.conf.json`, `package.json` and `src-tauri/Cargo.toml`. A PR that bumps them will
  be asked to revert it.

One more that's easy to trip over: HyprSpace runs Claude on the user's own subscription by spawning
their already-logged-in `claude` CLI. Don't add a custom claude.ai OAuth flow, and don't read or
forward a subscription token to call the Anthropic API directly. Reading credential files for
display-only fields like email or plan is fine.

## Checks before you push

```bash
npm run typecheck     # tsc --noEmit, frontend
npm run lint          # eslint
cd src-tauri && cargo check
```

`npm run lint` currently reports warnings and no errors. Warnings are fine to leave, but don't add
new ones. There's no test suite yet, so run the app and exercise whatever you changed. TS
hot-reloads, Rust changes trigger a recompile and relaunch.

## Opening a PR

1. Branch off `main`.
2. Keep the diff scoped to one thing. Don't reformat or "improve" adjacent code.
3. Run the checks above.
4. Open the PR against `main` and fill in the template. Say what changed, why, and how you verified
   it. Screenshots help for UI changes.

For anything large or architectural, open an issue first so we can agree on the shape before you
write it.

## Reporting bugs and vulnerabilities

Bugs go to GitHub issues using the bug report template. Security issues do **not**. See
[SECURITY.md](./SECURITY.md).
