# HyprSpace website

The marketing site. It's a standalone Vite + React + Tailwind app, separate from the desktop app,
with its own `package.json` and `node_modules`. It uses bun rather than npm.

```bash
cd website
bun install
bun run dev
```

Build:

```bash
bun run build          # tsc -b && vite build, output goes to website/dist/
```

`website/dist/` is gitignored, so deploy it wherever you host the site.

## Layout

- `src/site.ts` holds every outbound URL on the site: the repo, the releases page, and the
  Windows/macOS download links. Change them here, not inline in the components.
- `src/components/site/` has the page sections plus the `AppShell` that recreates the desktop app's
  titlebar and rail.
- `src/components/brainless/` is the agent-terminal UI. These come from the **@brainless** shadcn
  registry (<https://brainless.swerdlow.dev>), configured in `components.json`:

  ```bash
  bunx shadcn@latest add @brainless/<name>
  ```

  Treat them as vendored. They're checked in, and re-adding a component overwrites any local edits.
- `src/assets/shots/` has product screenshots. `workspace.png` is also used by the root README.

The desktop app forbids Tailwind, but that rule is about `src/` at the repo root. This sub-project is
Tailwind on purpose.
