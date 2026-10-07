# Contributing to HyprSpace

Thanks for taking a look. HyprSpace is a Rust app on GPUI, built as one Cargo workspace.

## Getting set up

Prerequisites and the clone-to-running steps are in the
[README](./README.md#building-from-source). Once `cargo run -p hyprspace` opens a window, you're ready.

To actually launch agents you need whichever CLI you want to use already installed and logged in
(`claude` or `codex`). HyprSpace spawns them, it never authenticates on their behalf.

## Repo layout

[AGENTS.md](./AGENTS.md) is the project guide, with the rules, where code lives and how it works.
Read it first. The one-paragraph version:

- `apps/hyprspace` is the binary. It starts the engine, opens the window, and wires the two
  together.
- `crates/proto` is the typed channel between UI and engine, `crates/engine` does everything that
  isn't drawing (sessions, PTYs, git, usage, persistence), `crates/harness` drives Claude and Codex
  over their machine protocols, and `crates/ui` is the GPUI app. `theme`, `syntax` and `update`
  are what their names say.
- `docs/` is the deeper material, starting at [docs/README.md](./docs/README.md). The domain words
  are in [docs/internals/glossary.md](./docs/internals/glossary.md), and the decisions and their
  reasons in [docs/internals/](./docs/internals/).
- `website/` is the marketing site, a separate Vite app with its own
  [README](./website/README.md).

## Code style

The full list of hard constraints lives in [AGENTS.md](./AGENTS.md). The ones that bite contributors
most often:

- **Respect the boundary.** `proto` and `engine` never depend on GPUI, and `ui` never calls the
  engine directly. It sends a `Command` and handles the `Event` that comes back. Something new that
  has to cross gets a type in `proto`.
- **No hard-coded colors in `ui`.** Use the tokens in `crates/theme`. Every theme has a light and a
  dark side, so a line or fill is the theme's ink at an alpha, never a literal white or black.
- **Don't orphan processes.** Every PTY and agent CLI has to die when its session closes and when
  the app quits, or ConPTY hosts pile up and burn CPU.
- **User text never goes into a command line.** Launch commands are built from fixed flags and
  catalog ids. Prompts are typed in after the CLI is up or passed through an environment variable.
- **Comment the why, not the what.** Match the surrounding code rather than your own preferred
  style, and use the words in `docs/internals/glossary.md`.
- **Never hand-edit version numbers.** `deploy.ps1` owns the `version` in the root `Cargo.toml`
  and the workspace entries in `Cargo.lock`. A PR that bumps them will be asked to revert it.

One more that's easy to trip over: HyprSpace runs Claude on the user's own subscription by spawning
their already-logged-in `claude` CLI. Don't add a custom claude.ai OAuth flow, and don't read or
forward a subscription token to call the Anthropic API directly. Reading credential files for
display-only fields like email or plan is fine.

## Checks before you push

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

CI runs the same three on Windows and macOS. The harness tests drive each adapter against a fake
CLI, so they need no agent installed. UI changes aren't covered by tests: run the app and exercise
whatever you changed.

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
