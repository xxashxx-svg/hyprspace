# HyprSpace project guide

> Read this first. It's the canonical guide for anyone (human or AI agent) working on this repo.
> Deep dives live in [`docs/`](./docs/README.md).

> **History:** HyprSpace was rebuilt from a Tauri + React app into this native GPUI app in
> October 2026. [`docs/REWRITE.md`](./docs/REWRITE.md) records why, the phases, and what was left
> behind; the last Tauri release is the `v0.21.1` tag.

HyprSpace is a **native desktop workspace for coding agents**, written in Rust on GPUI (Zed's GPU
UI framework). Each folder is a **space** in the sidebar, and each conversation in it is a
**thread**. A thread runs as a **structured session** (Claude or Codex driven over its machine
protocol and drawn as a transcript with tool calls, approvals and diffs) or as a **terminal
session** (a real PTY running a shell, or an agent CLI interactively). One thread shows at a
time, with a files and git dock, a read-only file viewer that opens over it, a usage meter and a
command palette.
The words are defined once in [`docs/CONTEXT.md`](./docs/CONTEXT.md); use exactly those.

- **Stack:** Rust 1.98.1 (edition 2024) · GPUI and `gpui_platform` pinned to one upstream Zed
  commit · `alacritty_terminal` for the emulator · `portable-pty` (ConPTY on Windows) · tokio in
  the engine · `pulldown-cmark` for markdown · tree-sitter for the viewer · self-update with the
  minisign key the Tauri app used.
- **Platforms:** Windows (primary, built locally) + macOS on Apple silicon (built in CI). Linux was
  dropped on 2026-10-02 because nobody used it; don't add Linux builds or Linux-only work.
- **Companion app:** [`mobile/`](./mobile/README.md) is an Expo/React Native Android app. It paired
  with the Tauri app's LAN bridge, which left with the Tauri app, so it has nothing to pair with
  until it is redone (docs/REWRITE.md). Its own app, its own versioning.

---

## Critical constraints: do not violate

Rule 1's wording predates structured sessions and still names Tauri-era files; Ash will update it.

1. **Subscription compliance (most important).** All *inference* runs on the user's
   **subscription** by spawning their already-logged-in `claude` CLI — nothing else. **NEVER** build
   a custom claude.ai OAuth flow, **NEVER** feed a subscription token to an SDK, and **NEVER** use
   one to send a prompt or complete anything. The terminal panes just spawn the CLI.
   The one sanctioned exception is `devtools/live_usage.rs`: it reads the token the CLI already
   stores and sends it back to that same provider's **usage endpoint only**
   (`api.anthropic.com/api/oauth/usage`, `chatgpt.com/backend-api/codex/usage`) to read the
   account's own limits for display. The token is read per request, never stored or forwarded
   anywhere else. Respect the poll cadence in that file: Claude's bucket is shared with Claude Code
   itself, so 180s is the floor.
2. **Keep the boundaries.** `proto` and `engine` never depend on GPUI, and `ui` reaches the engine
   only through the typed channel in `hyprspace-proto` (commands in, events out). A crate that
   wants to cross that line gets a new type in `proto` instead. Only `apps/hyprspace` sees both
   sides. Reasons: [docs/adr/0002](./docs/adr/0002-channel-boundary.md).
3. **Colors come from `crates/theme`.** No hard-coded colors anywhere in `ui`. The tokens and all
   six themes are ported from the Tauri app's `tokens.css` and `themes.ts`: each theme is one hue
   with a light and a dark side derived from it (in oklch), so never write a literal white or black
   wash. A line or fill is the theme's ink at an alpha, which is white-alpha on dark and
   black-alpha on light. Buttons and highlights use the accent tokens; accent goes on the one
   primary action. Match the existing neutral, low-contrast look.
4. **Terminal stability.** PTYs must be killed on exit (they are: `Engine::shutdown` runs the PTY
   manager's `kill_all`) or ConPTY hosts (`OpenConsole.exe`) orphan and burn CPU. `Root` keeps one
   view per thread (`views`); don't rebuild a terminal view and its emulator when the thread on
   screen changes.
5. **Version numbers are managed by `deploy.ps1` only.** Never hand-edit the `version` in the root
   `Cargo.toml`'s `[workspace.package]` (every crate inherits it) or the workspace entries in
   `Cargo.lock`. See [docs/VERSIONING.md](./docs/VERSIONING.md).
6. **Never ship unless the user explicitly asks.** Do NOT run `deploy.ps1` / publish a release on
   your own — multiple agents may be working at once, and a surprise release is hard to undo. Same
   for commit/push: only when asked. Otherwise work with `cargo run`; `main` is default, branch
   before committing if asked.
7. **Release notes are written at ship time, not per task.** Don't keep a running changelog while you
   work. When the user asks to ship, look at what changed since the last release
   (`git log <lastTag>..HEAD`) and write a few short user-facing bullets — pass them as the
   `deploy.ps1` notes; it records them in `docs/CHANGELOG.md` and the in-app "What's new".
8. **Analytics stay boring and honest.** The GPUI app sends no analytics: no events, no install
   id. The Tauri app's one PostHog event (`app_opened`) was left behind with it. The only requests
   the app makes on its own are the update check (`latest.json` on GitHub releases, then the
   installer it names) and the usage endpoints of rule 1. This repo is public: adding analytics is
   Ash's call, needs an off switch in Settings, and the Settings copy must say exactly what is sent
   in the same commit. Never send prompts, terminal output, paths, project names, or anything
   joined to an account.
9. **Code style:** clear and conventional over casual. Comment the why, not the what. UI code goes
   in a folder per area (`crates/ui/src/composer`, `crates/ui/src/dock`), one concern per file; a
   file past about 600 lines is a sign to split it. A choice someone would later question gets a
   short entry in `docs/adr/`. Dead code leaves with the change that orphans it.
10. **Copy:** every string a user reads is plain English. No em dashes, no curly quotes, no
   filler. One idea per sentence. Say what happens, not how it feels.

---

## Run, build and check (quick reference)

```bash
./scripts/dev.ps1                     # the dev loop: rebuilds on save and swaps the running app
cargo run -p hyprspace                # the app, debug build (dependencies build at opt-level 2)
cargo run -p hyprspace -- <folder>    # open a folder as a space, the way `code .` does
cargo build --release -p hyprspace    # target/release/hyprspace(.exe)
./scripts/package-windows.ps1         # release build + NSIS installer in target/package
bash scripts/package-macos.sh         # on a Mac: HyprSpace.app, its .app.tar.gz and a dmg
```

**The check** (CI runs it on Windows and macOS for every push to `main` and `rewrite`,
`.github/workflows/check.yml`; a red check blocks a merge):

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

UI changes are checked by running the app. A dev build shares `~/.hyprspace/native` with an
installed copy; set `HYPRSPACE_STATE_DIR` to a scratch folder to keep them apart. `scripts/dev.ps1`
does that for you (`~/.hyprspace/dev`, seeded from your threads with none open), and keeps the
running dev app until a build succeeds. Windows dev builds link with `rust-lld`
(`.cargo/config.toml`) and dependencies carry no debug info, which took a one-file change from
about 14 s to 7 s.

Releases are cut by `deploy.ps1`: it bumps the workspace version and `Cargo.lock`, writes the
changelog entry, commits and tags, opens a draft GitHub release with the notes, and runs
`.github/workflows/release.yml`. CI builds Windows and macOS with `scripts/package-*`, signs what
the updaters install with the key held in the repo's secrets, merges each into `latest.json`, and
publishes the draft when both are in. No machine needs the signing key. A dry run touches no
release: `gh workflow run release.yml --ref <branch> -f dry_run=true`.

---

## Repo map

```
Cargo.toml                   the workspace: members, the app version, GPUI's pinned rev
apps/hyprspace/              the binary: starts the engine, opens the window, `agent-hook` and
                             `status-line` subcommands for Claude's hooks; assets/ (icons),
                             package/ (NSIS script, macOS Info.plist)
crates/
  proto/                     the channel: Command, Event, RunEvent, Launch, AppState, Pane, folder,
                             usage, skills and update messages. No GPUI
  harness/                   Harness trait + claude/ (stream-json) and codex/ (app-server)
                             adapters, catalog.rs (models and efforts), fixtures/fake_cli (tests)
  engine/                    everything that isn't drawing. No GPUI. pty.rs, terminal.rs (agent
                             launch commands), hooks.rs (Claude hook listener), journal.rs,
                             requests.rs (state, agents, resume list, clone), folder.rs + git/,
                             open.rs, usage/ (live.rs, local.rs, status.rs), providers.rs,
                             sessions.rs, skills.rs, persist.rs, legacy.rs (Tauri state import),
                             update.rs, env.rs (PATH rebuild, Ctrl+C, Claude session markers)
  ui/                        the GPUI app: root/, sidebar/, composer/, transcript/, terminal/,
                             workbench/, dock/, viewer/, palette/, settings/, skills/, usage/,
                             update/, intro/, markdown/, input/ (text box with IME), widgets.rs
  theme/                     tokens and the six themes, light and dark, as plain data
  syntax/                    tree-sitter highlighting for the viewer. No GPUI
  update/                    feed, signature check, install; examples/verify.rs for CI
scripts/                     package-windows.ps1, package-macos.sh, ci-build-latest.mjs
                             (writes latest.json), check-windows-install.ps1, the upgrade test's
                             check-upgrade-*.{ps1,sh} and tauri-autoinstall.patch, logo.svg
mobile/                      the Android app, its own Expo + React Native project (see its README)
website/                     the marketing site, its own Vite + React + Tailwind app (bun)
docs/                        documentation (start at docs/README.md); adr/ holds the decisions
.github/workflows/           check.yml, release.yml, upgrade-test.yml
CONTRIBUTING.md              dev setup, style rules, PR flow (for outside contributors)
```

---

## Architecture in one screen

- **Engine and UI.** `apps/hyprspace` starts the engine (`Engine::start`, its own two-worker tokio
  runtime) and hands the two channel ends to `ui::Root`. The UI sends `Command`s, fire and forget;
  the engine answers with `Event`s, each naming the session or request it is about. The command
  loop handles one command at a time, so keystrokes reach a PTY in order; slow work (git, clones,
  `--version` checks) goes to the blocking pool.
- **Spaces and threads.** `proto::state::AppState` holds the spaces (one folder each, no open
  spaces since ADR 0008), their threads (each with the `Launch` it resumes with), the composer's
  picks and the appearance. The UI owns its shape; the engine saves it
  whole to `~/.hyprspace/native/state.json`. On a first run the engine imports the Tauri app's
  `~/.hyprspace/v2`, read only (ADR 0012).
- **Structured sessions.** `harness::Harness::start` returns a `Session` that owns the CLI process:
  `send` starts a run or steers the live one, `interrupt` ends it, `answer` replies to an
  approval, drop kills the CLI. Claude runs as `claude --print --input-format stream-json
  --output-format stream-json --verbose --permission-prompt-tool stdio ...`; Codex as its
  app-server over JSON-RPC. Both become `RunEvent`s, with exactly one `Finished` per run. Each
  thread's events are appended to a journal and replayed after a restart (ADR 0004, 0005).
- **Terminal sessions.** `engine/src/pty.rs` spawns a bare shell; `terminal.rs` builds the agent's
  launch command from fixed flags and catalog ids and types it into the shell once it first
  prints. User text never goes into a command line: Claude gets the prompt typed in at its first
  status line, Codex and Gemini read it from `HYPRSPACE_PROMPT`. Claude's hooks re-invoke our
  binary (`hyprspace agent-hook`), which posts to a loopback listener for the sidebar's live state
  (ADR 0006). The UI's `terminal/` folds bytes through `alacritty_terminal` and paints the grid.
- **Composer.** Agent, model, effort, permission (`Plan`, `Ask`, `Auto`, `Bypass`, mapped per CLI
  in ADR 0004), structured or terminal, the resume list (the CLI's saved conversations for the
  folder), and a clone card when the text starts with a repository link.
- **Main area, dock and viewer.** One thread on screen at a time; clicking a sidebar row shows
  it (ADR 0015). The dock (Ctrl+Shift+G) has the file tree and the git tab; a file or a diff opens
  in a read-only card over the window, and a Ctrl+clicked image in a zoomable lightbox. Folder
  work rides `Command::Folder` behind one git lock (ADR 0007).
- **Usage.** `engine/src/usage/live.rs` reads each provider's usage endpoint (rule 1) and answers
  inside the floor (180s Claude, 60s Codex) from its last reading, backing off on 429 and 5xx. One
  `ui::usage::Limits` entity holds every reading for the ring and Settings, and falls back to
  Claude's status-line reports and Codex's session files (ADR 0009).
- **Updates.** `crates/update` reads the same `latest.json` and verifies with the same key the Tauri
  app used. Only an installed copy updates itself; `cargo run` builds never do (ADR 0010, 0011).

Full design details: **[docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md)** and
**[docs/adr/](./docs/adr/)**.

---

## Gotchas learned the hard way

- **`claude --resume <id>` is folder-scoped.** A conversation only resumes in the directory it was
  created in, so the Claude harness looks the id up under `~/.claude/projects/` and spawns in the
  folder its transcript records, whatever `cwd` the caller passed.
- **Never type into an agent CLI on a timer.** An Enter meant for a Codex prompt once landed on
  Codex's "update available" dialog and upgraded the user's codex. Type only after a real signal
  (Claude's status line), or pass the prompt at start.
- **ConPTY hosts orphan.** Every exit path has to end in `Engine::shutdown`. Check for leftover
  `OpenConsole.exe`, shells and CLIs after closing the app.
- **A `.cmd` shim runs through `cmd /c`.** Killing `cmd` can leave the node child behind; the
  native `claude.exe` is unaffected.
- **Ctrl+C can arrive ignored.** A parent that started us with Ctrl+C ignored passes that on to
  every shell; `env.rs` takes it back at startup.
- **`secondary` in a key binding** is Ctrl on Windows and Cmd on macOS. The palette's Ctrl+K is
  bound everywhere, terminals included, so a shell never sees its kill-line Ctrl+K.
- **Checks without side effects.** `HYPRSPACE_OPEN_LOG` logs editor and Explorer launches instead
  of running them, and `HYPRSPACE_USAGE_FIXTURES` reads usage from files instead of spending the
  request bucket Claude Code shares.
- **The update feed and key are compile-time only** (`HYPRSPACE_UPDATE_FEED`,
  `HYPRSPACE_UPDATE_PUBKEY` through `option_env!`), so a shipped build can't be pointed elsewhere.
- **GPUI lays out every uncached view on every frame.** One terminal's output used to redo the
  whole sidebar's layout (18 ms of a 20 ms frame, at 179 Hz). Big views are embedded with
  `.cached(style)` (terminals in `workbench/frame.rs`, the sidebar in `root/render.rs`) and must
  `cx.notify()` on anything that changes how they look, focus included. Long lists are a virtual
  `gpui::list` (the sidebar), so only rows on screen are laid out.
- **Persisted names** (state files, journals) are sanitized to a token so they can't leave their
  folder. Journals are never trimmed yet.

## Docs index
- [docs/README.md](./docs/README.md): index of everything below
- [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md): how the tricky subsystems work
- [docs/CONTEXT.md](./docs/CONTEXT.md): the domain words
- [docs/adr/](./docs/adr/): decisions and their reasons
- [docs/VERSIONING.md](./docs/VERSIONING.md): when to bump which digit
- [docs/BUILD-MAC.md](./docs/BUILD-MAC.md): building the macOS app locally
- [docs/REWRITE.md](./docs/REWRITE.md): the GPUI rewrite, as history
