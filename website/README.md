# HyprSpace website

The marketing site. It's a standalone Vite + React + Tailwind app — separate from the desktop app,
with its own `package.json` and `node_modules`. It uses **bun**, not npm.

```bash
cd website
bun install
bun run dev
```

Build:

```bash
bun run build          # tsc -b && vite build → website/dist/
```

`website/dist/` is gitignored; deploy it wherever you host the site.

## Layout

- `src/site.ts` — **every outbound URL** on the site (releases, repo, the Windows/macOS download
  links). `REPO` still points at the old private repo URL — update it to the real public repo.
- `src/components/site/` — the page sections.
- `src/components/brainless/` — the agent-terminal UI. These come from the **@brainless** shadcn
  registry (<https://brainless.swerdlow.dev>), configured in `components.json`:

  ```bash
  bunx shadcn@latest add @brainless/<name>
  ```

  Treat them as vendored: they're checked in, and re-adding a component overwrites local edits.
- `src/assets/shots/` — product screenshots (`workspace.png` is also used by the root README).

Note the desktop app forbids Tailwind — that rule is about `src/` at the repo root. This
sub-project is Tailwind, and that's intentional.
