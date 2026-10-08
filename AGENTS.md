# HyprSpace

HyprSpace is a native desktop workspace for coding agents, written in Rust on GPUI (Zed's GPU UI
framework). Each folder is a space, each conversation in it is a thread, and a thread runs Claude
or Codex as a structured session (driven over the CLI's machine protocol and drawn as a
transcript) or as a terminal session (a real PTY running a shell, or an agent CLI interactively).
One thread shows at a time, with a files and git dock, a file viewer that edits text, a usage meter
and a command palette.

It ships for Windows (primary) and macOS on Apple silicon. Linux was dropped in October 2026; don't
add Linux builds or Linux-only work.

## What we never compromise on

### 1. The user's own subscription

All inference runs on the user's subscription, through the `claude` and `codex` CLIs they already
installed and logged in to. Structured sessions drive those same binaries over stdio; terminal
sessions type them into a shell. **Never** build a claude.ai OAuth flow, **never** hand a
subscription token to an SDK, and **never** use one to send a prompt.

The one exception is `crates/engine/src/usage/live.rs`. It reads the token the CLI already stores
and sends it to that provider's **usage endpoint only** (`api.anthropic.com/api/oauth/usage`,
`chatgpt.com/backend-api/codex/usage`) to show the account's own limits. The token is read per
request and never stored or sent anywhere else. Claude's request bucket is shared with Claude Code
itself, so it is asked at most once every 180 seconds; the engine enforces that floor.

### 2. Boring, honest networking

The app sends no analytics: no events, no install id. The only requests it makes on its own are
the update check (`latest.json` on GitHub releases, then the installer it names) and the usage
endpoints above. This repo is public. Adding analytics is Ash's call, needs an off switch in
Settings, and the Settings copy must say exactly what is sent. Never send prompts, terminal output,
paths, project names, or anything joined to an account.

The phone bridge is the one way in. It listens only once the user switches it on in Settings,
Phone, speaks TLS under a certificate the phone pins, and answers only phones the user paired.
It never goes through a server of ours, and its Settings copy says what a phone sees and can do.

### 3. Fast on a busy machine

Ash runs many agents at once, all day. A dropped frame, a lying spinner or a stale label gets
noticed. GPUI lays out every uncached view on every frame, so big views are `.cached(...)` and
long lists are a virtual `gpui::list`. No animation that repaints forever.

## A note from Ash

I love to build, and I build complex things as simply as possible. Find the smallest change that
makes the right behavior obvious. Don't keep complexity because it already exists, and don't add
machinery because it looks impressive. Fight scope creep.

Treat this file as good defaults, not law. The developer you're working with can override any of
it. If a rule here fights the task in front of you, say so before breaking it.

## A small glossary

Use these words in code, UI copy and docs. The full list is in
[docs/internals/glossary.md](docs/internals/glossary.md).

- **space**: one folder and the threads that run in it. Not "workspace" (that's the Cargo one).
- **thread**: one conversation in a space, one row in the sidebar.
- **structured session** / **terminal session**: the two ways a thread runs.
- **run**: one prompt in a structured session, from sending it to the CLI's result. Not "turn".
- **harness**: the adapter that drives one agent CLI over its machine protocol.
- **engine** / **UI** / **channel**: everything but drawing, the GPUI app, and the typed queue
  between them.
- **agent**: a coding CLI the user has (`claude`, `codex`). **provider** when the subject is the
  account behind it.

## The ways to hurt yourself

1. **Touching the live app.** Ash is usually working in the installed HyprSpace while you work. A
   dev build shares its state folder, `~/.hyprspace/native`, unless `HYPRSPACE_STATE_DIR` points
   elsewhere (`scripts/dev.ps1` does this). Never kill a HyprSpace, shell or agent process by name.
   Stop only what you started, by the PID you captured.
2. **Running Ash's agents.** Test state copied from real state must not open any thread, or the
   copy launches an agent on a conversation that may be live in the installed app.
3. **Orphaned ConPTY hosts.** Every exit path ends in `Engine::shutdown`, which kills every PTY.
   An orphaned `OpenConsole.exe` spins at about 8% CPU forever.
4. **Typing into an agent CLI on a timer.** An Enter meant for a Codex prompt once landed on
   Codex's "update available" dialog and upgraded the user's Codex. Type only after a real signal
   (Claude's status line), or pass the prompt at start. User text never goes into a command line.
5. **Spending Claude's usage bucket.** `HYPRSPACE_USAGE_FIXTURES` reads usage from files, and
   `HYPRSPACE_OPEN_LOG` logs editor and Explorer launches instead of running them.

## Hit every surface

The most common miss is a change that works on the path you tried and nowhere else. Before calling
work done, walk this list:

- **Session kinds.** Structured and terminal threads differ in nearly everything. Decide for both.
- **Agents.** Claude and Codex each have a harness, a launch command and their own files. A
  provider-shaped feature needs a decision per agent, even if it's "not for Codex".
- **Platforms.** Windows and macOS. `secondary` in a key binding is Ctrl on one and Cmd on the
  other.
- **Themes.** Six themes, each with a light and a dark side. Check both sides.
- **Entry points.** A thing reachable from a menu is often also in the command palette, Settings or
  a key binding. Settings, Shortcuts is a hand-kept list: a changed binding changes there too.
- **Reverse states.** Settle needs bring back, snooze needs wake, a way in needs a way out.
- **The phone.** The phone sees the sidebar through the board (`crates/ui/src/phone`) and acts
  through the same code as a click. A new thread state, row line or action may need the board,
  `crates/proto/src/phone.rs` and the Android app (`mobile/`). A changed wire type moves
  `PROTOCOL` and the fixtures the proto test writes.
- **Saved state.** `AppState` is saved whole. A changed or removed field must still load an old
  `state.json` (serde defaults, or a rewrite before parsing like `requests::without_gemini`).
- **Docs.** Check whether the change makes existing guidance wrong. See Documentation below.

## Run, check, ship

```bash
./scripts/dev.ps1                     # the dev loop: rebuilds on save, own state folder
cargo run -p hyprspace -- <folder>    # one run, opened on a folder
```

The check. CI runs it on Windows and macOS for every push to `main`:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

The Android app has a check of its own, run from `mobile/` (CI runs it on Linux):

```bash
./gradlew testDebugUnitTest assembleDebug
```

- Prove a change the smallest way that works: the tests you touched, then the check before a
  commit. UI is checked by running the app.
- Test logic and behavior at the edges: harnesses against `crates/harness/fixtures/fake_cli`,
  engine logic with unit tests. Don't add tests that mirror the implementation.
- **Never commit, push or ship unless asked.** Several agents may be working at once, and a
  surprise release is hard to undo.
- **Versions belong to `deploy.ps1`.** Never hand-edit the workspace `version` in `Cargo.toml` or
  the workspace entries in `Cargo.lock`.
- **Release notes are written at ship time.** Don't keep a running changelog. When asked to ship,
  read `git log <lastTag>..HEAD` and write a few short user-facing bullets for `deploy.ps1`. See
  [docs/operations/release.md](docs/operations/release.md).

## How it works

`apps/hyprspace` starts the engine on its own tokio runtime, opens one GPUI window with `ui::Root`,
and hands the UI the two ends of the channel in `crates/proto`: `Command`s in, `Event`s out. The
engine's command loop takes one command at a time, so keystrokes reach a PTY in order, and slow work
goes to the blocking pool. A structured session is a `harness::Session` that owns the CLI process
and turns its output into `RunEvent`s, journaled per thread and replayed after a restart. A
terminal session is a PTY whose agent launch command the engine types in once the shell is up; the
UI folds its bytes through `alacritty_terminal`. The UI owns `AppState`'s shape; the engine saves it
to `~/.hyprspace/native/state.json`.

Architecture and its reasons: [docs/internals/overview.md](docs/internals/overview.md).

## Where code lives

- `apps/hyprspace`: the binary. Starts the engine, opens the window, and serves the `agent-hook`
  and `status-line` subcommands Claude's hooks call. Installer and bundle files in `package/`.
- `crates/proto`: the channel's commands, events and the records they carry. No GPUI.
- `crates/harness`: the Claude (stream-json) and Codex (app-server) adapters, and the model
  catalog.
- `crates/engine`: everything that isn't drawing: PTYs, launches, hooks, journals, saved state,
  git, usage, updates. No GPUI.
- `crates/ui`: the GPUI app, one folder per area.
- `crates/theme`, `crates/syntax`, `crates/update`: tokens and themes, tree-sitter highlighting,
  the updater.
- `mobile/`: the Android app, Kotlin and Compose. It pairs with the desktop over the phone
  bridge (`crates/engine/src/phone`). Its own Gradle project and version.
- `website/` is the marketing site, its own project.

## Taste

- **Keep the boundary.** `proto` and `engine` never depend on GPUI, and `ui` reaches the engine
  only through the channel. Something new that has to cross gets a type in `proto`.
- **Colors come from `crates/theme`.** No literal colors in `ui`. A line or fill is the theme's ink
  at an alpha, white on dark and black on light. Accent goes on the one primary action. Match the
  neutral, low-contrast look that's there.
- **One concern per file**, in a folder per area (`crates/ui/src/composer`, `dock`). A file past
  about 600 lines wants splitting.
- **Comment the why, not the what.** Dead code leaves with the change that orphans it.
- **Copy is plain English.** No em dashes, no curly quotes, no filler. One idea per sentence. Say
  what happens, not how it feels.
- **Keep one view per thread.** `Root.views` holds them, so switching threads never rebuilds a
  terminal and its emulator.

## Documentation

Most changes need no doc change. Agents can read the code.

- `docs/internals/` holds decisions and their reasons, constraints that span crates, and traps
  the source doesn't show. Before adding a paragraph, ask what a maintainer would get wrong without
  it. If reading the code answers it, leave it out.
- Don't list files, fields or methods, narrate control flow, or keep a history of what changed.
  Git keeps history.
- When a documented decision changes, rewrite the text. Don't append the new story after the old.
- `docs/operations/` holds the dev setup and the release.
- Don't commit plans, research notes or scratch files.
