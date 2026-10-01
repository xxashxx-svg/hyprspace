# HyprSpace native rewrite (GPUI)

The brief for rebuilding HyprSpace as a native Rust app on GPUI, with structured agent sessions as
the main experience. Decided by Ash on 2026-10-02. Read this whole file before touching the rewrite.

## Decisions

- **UI: GPUI** (Zed's GPU UI framework, Rust). It replaces React, Tauri and the webview.
- **Sessions are structured first.** An agent session renders as a transcript (messages, tool
  calls, approvals, diffs), driven by the CLI's machine protocol. **Terminal stays a session type**,
  for plain shells and for running an agent CLI interactively (see Billing for why it must stay).
- **New app beside the old, in this repo.** The Tauri app gets bug fixes only until the GPUI app
  reaches parity (checklist below), then it is deleted.
- **Mobile is redone later.** The Android app may break during the rewrite.
- **Windows and macOS only.** Linux was dropped on 2026-10-02; the GPUI app never targets it.
- **zeron is the reference.** [zeronsh/zeron](https://github.com/zeronsh/zeron) built this exact
  product on GPUI. It is MIT, so adapting its code is allowed with attribution in a
  `THIRD_PARTY_NOTICES.md`. Clone it next to this repo and read its `ARCHITECTURE.md`,
  `docs/research/`, and `crates/harness` before designing anything.

## Steps

Work through every phase in order; each ends on its completion criterion. After each phase,
commit, tick what's done, and note progress under Spike results or the parity checklist, so a
fresh session can pick up exactly where the last one stopped.

1. **Spike (about 2 days).** One GPUI window on Windows with (a) one structured Claude session that
   streams a reply from the headless CLI and renders it, and (b) one terminal pane on
   `alacritty_terminal` fed by our existing PTY code running `claude`. Measure RAM with 4 sessions
   open against the Tauri app with 4 panes.
   *Done when* both run on Windows and the RAM numbers are written below under Spike results.
   Carry on to phase 2 unless the spike hits a real blocker (something GPUI can't do on Windows);
   only a blocker goes back to Ash.
2. **Workspace and engine.** Create the Cargo workspace (layout below) and move the reusable Rust
   into crates, each with tests. Add the CI check from Code standard. The Tauri app keeps building
   throughout.
   *Done when* the check passes on Windows and macOS in CI and `npm run tauri build` still succeeds.
3. **Harness.** One `Harness` trait with Claude (stream-json) and Codex (app-server) adapters:
   start, send, steer, interrupt, approvals, resume, images, model and effort.
   *Done when* each adapter passes tests against a fake CLI fixture (zeron's
   `crates/harness/tests/fixtures/fake-claude.sh` pattern) and one real session per CLI completes.
4. **UI shell.** Sidebar (folders and threads with live status), composer (agent, model, effort,
   permissions, resume list, clone card), transcript, approval prompts.
   *Done when* a thread can be started, steered, approved and resumed from the UI for both CLIs.
5. **Terminal session type.** *Done when* it matches every terminal row of the parity checklist.
6. **Everything else on the parity checklist,** then packaging: installers, auto-update, CI for
   Windows and macOS.
   *Done when* every checklist row is ticked; then delete `src/`, `src-tauri/` and the npm setup.

## Code standard

The rewrite is the cleanup: nothing from `src/` is ported line by line, and the old app is never
tidied, only replaced. Every phase holds the new code to this bar:

- **The check:** `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and
  `cargo test --workspace`, run in CI on every push to `rewrite` for Windows and macOS. A red check
  blocks the phase.
- **Boundaries:** `proto` and `engine` never depend on GPUI; `ui` reaches the engine only through
  the typed channel. A crate that wants to cross that line gets a new type in `proto` instead.
- **Small modules:** one concern per file. A file past about 600 lines is a sign to split by
  concern, the way the old `App.css` got split into per-area sheets.
- **Shared vocabulary:** `docs/CONTEXT.md` defines the domain words (space, thread, session,
  harness, run) once, and code, UI copy and docs use exactly those words (zeron's `CONTEXT.md`).
- **Decisions on record:** a choice someone would later question (GPUI rev, a protocol shape, a
  dropped feature) gets a short entry in `docs/adr/` with the reason.
- **Tests at the edges:** each harness adapter runs against a fake CLI fixture; engine logic gets
  unit tests; UI is checked by running it.
- **Style:** CLAUDE.md's code style and copy rules apply: comment the why, plain English strings.
- **Dead code leaves with the commit that orphans it.** Clippy's warnings-as-errors catches the rest.

## Billing and compliance

Facts as of 2026-10-02. Recheck the sources before shipping structured Claude sessions.

- Structured Claude runs **the user's own `claude` binary** headless:
  `claude --print --input-format stream-json --output-format stream-json --permission-prompt-tool stdio`
  (zeron: `crates/harness/src/claude/mod.rs`). Never the Agent SDK package, never an API key we
  hold, never a token we read. CLAUDE.md rule 1 still governs; Ash updates its wording for this
  mode, an agent does not.
- Today headless `claude -p` draws from the plan's normal limits. Anthropic announced, then paused
  on 2026-06-15, a split that would move headless, SDK and third-party use to a separate monthly
  credit at API rates ($20 Pro, $100 Max 5x, $200 Max 20x), while interactive Claude Code keeps the
  full plan limits. That is why the terminal session type stays first-class.
  Sources: [Anthropic support](https://support.claude.com/en/articles/15036540-use-the-claude-agent-sdk-with-your-claude-plan),
  [Zed's summary](https://zed.dev/blog/anthropic-subscription-changes),
  [Agent SDK overview](https://code.claude.com/docs/en/agent-sdk/overview).
- Codex runs through its app-server (JSON-RPC over stdio), which exists for apps like this.

## Architecture

```
Cargo.toml            workspace (src-tauri stays outside it until the Tauri app is deleted)
crates/
  proto/              wire and domain types shared by everything: sessions, events, tool calls
  harness/            Harness trait + claude/, codex/ adapters, process spawning, fake-CLI tests
  engine/             sessions (pub/sub, run journal, crash recovery), PTYs, git and worktrees,
                      providers, usage, persistence. No UI, fully testable headless
  ui/                 the GPUI app: shell, sidebar, composer, transcript, terminal view, dock
  theme/              tokens, light and dark variants
apps/hyprspace/       the binary
```

- The engine runs in-process behind a typed channel boundary, so a headless mode or the phone can
  attach later without rework (zeron's "headed or headless" design, minus its sync).
- **GPUI source:** upstream Zed at `20d29fc6bc2fc2b58d1fff8d8e0503b9ba7f41d8` (`main` on
  2026-10-02) for `gpui` and `gpui_platform`, set once in the root `Cargo.toml` (the spike also
  used `gpui_tokio`; phase 2 dropped it, see [adr/0002](./adr/0002-channel-boundary.md)).
  Every Zed crate it pulls in is Apache-2.0, it built and ran on Windows with no patches, and
  zeron's fork `zeronsh/zui` only adds visual effects we don't need. Reasons in
  [adr/0001-gpui-source.md](./adr/0001-gpui-source.md).
- **License boundary:** HyprSpace is MIT. Zed's `editor`, `ui`, `theme`, `markdown` and
  `terminal_view` are GPL-3.0; they stay out. Build the terminal on `alacritty_terminal`
  (Apache-2.0) and markdown on `pulldown-cmark`, as zeron does.

## Reuse from the Tauri app

Move into crates, keeping their hard-won behaviour and comments:

- `src-tauri/src/pty.rs`: PTY manager, byte coalescing, `kill_all` on exit (ConPTY hosts orphan
  otherwise).
- `src-tauri/src/agenthook.rs`: Claude hook listener, still needed for terminal sessions.
- `src-tauri/src/devtools/`: git, worktrees, providers (install and sign-in checks), usage,
  `live_usage.rs` (the 180s Claude poll floor is a hard rule), skills, sessions (resume list).
- `src-tauri/src/persist.rs`, the PATH rebuild and `drop_claude_session_env` in `lib.rs`.
- Product decisions in `src/` worth carrying: the composer flow, the clone card's
  folder and open-in choices, the paste tray, model and effort catalog (`src/lib/models.ts`),
  copy rules (CLAUDE.md rule 10), analytics rule 8.

Leave behind: the licensing, paywall and Supabase auth chain (unused), `git_root` (dead), the
launcher remnants, and anything only the webview needed.

## Parity checklist

Tick each row in the GPUI app before deleting the Tauri app.

- [ ] Sidebar: folders and threads, live status, archive, rename, search, right-click menu
- [ ] Composer: agent, model, effort, permission mode, resume past sessions, clone with choices
- [ ] Structured transcript: streaming markdown, tool calls, approvals, diffs, images in prompts
- [ ] Terminal sessions: selection, scrollback, search, links, ctrl+click paths, paste images
- [ ] Panes or tabs for several sessions at once
- [ ] Dock: files tree, git stage, commit, push, diff view
- [ ] File viewing (CodeMirror replacement, or open in the external editor at first)
- [ ] Usage meter with the live limits rules
- [ ] Command palette
- [ ] Settings: appearance (themes, light and dark), defaults, usage, skills, general
- [ ] Intro
- [ ] Open in editor or Explorer/Finder
- [ ] Installers, auto-update, CI release for Windows and macOS

## Open questions for Ash

- Gemini, OpenCode, Grok: structured adapters (zeron has `acp`, `opencode`, `pi`) or terminal only?
- Multi-device sync like zeron: out of scope for the first release unless Ash says otherwise.

## Spike results

Run on 2026-10-02, Windows 11, RTX 3050, claude 2.1.287. No blocker: phase 2 can start.

**What exists.** A root Cargo workspace (`Cargo.toml`, `src-tauri` and `mobile` excluded, so
`cargo check` in `src-tauri` still passes) with one binary, `apps/hyprspace`:

- `claude.rs` spawns the user's `claude` with `--print --input-format stream-json
  --output-format stream-json --verbose --include-partial-messages --permission-prompt-tool stdio`
  through `gpui_tokio`, writes one user turn, streams `text_delta`s into `chat.rs`, and denies
  any `can_use_tool` request (no approval UI yet). `--verbose` is required for stream-json.
- `pty.rs` is `src-tauri/src/pty.rs` cut down (coalescer, off-thread wait, `kill_all` on quit).
  `term/` folds its bytes through `alacritty_terminal`, answers terminal queries, paints the grid
  on a `canvas`, and maps keys to bytes. The shell types `claude` in, like the Tauri app does.
- `hyprspace [--chats N] [--terms N] [--prompt TEXT] [--launch CMD]` lays the sessions out as a
  grid. 10 unit tests; fmt, clippy `-D warnings` and tests pass.

Verified by running it: the chat pane streamed "Hey Ash. I'm Claude Opus 5.5" and showed "Done
in 2.0s"; the terminal pane booted the claude TUI in color, and text posted as key messages to
the window appeared in claude's prompt. Closing the window left no orphaned claude, shell or
conhost processes.

**RAM.** Release build, measured with `Get-Process` once sessions were idle. "App" is the app's
own processes: `hyprspace.exe` alone for GPUI, `hyprspace-tauri.exe` plus its WebView2 processes
for Tauri. Shells, CLIs, their children and the ConPTY hosts are left out of both.

| App | Sessions | App working set | App private bytes |
| --- | --- | --- | --- |
| GPUI spike | 2 structured + 2 terminals running claude | 70 MB | 98 MB |
| GPUI spike | 4 terminals running claude | 65 MB | 104 MB |
| GPUI spike | 20 terminals (shell + a file listing), all on screen | 84 MB | 163 MB |
| Tauri v0.21.1 | 20 terminal panes across 32 spaces | 745 MB | 1853 MB |

The Tauri numbers come from the installed app Ash already had open, measured without touching
it (up 1.5 days; the WebView2 renderer alone was 473 MB working set and 1369 MB private). It had
20 panes, not 4, so the GPUI app was also run with 20 terminals for a like-for-like count. A
second Tauri instance with 4 panes was not launched: it shares the window-state file and the
agent-hooks folder with the running one. At 20 sessions each, Tauri used 9x the working set and
11x the private bytes of the GPUI app.

**Not checked yet.** macOS build, IME and non-ASCII typing (the terminal reads key events only,
no input handler), mouse selection, scrollback, resize under load.

**Next (phase 2).** Move `pty.rs`, the emulator and the stream-json parsing into crates per the
layout above, add the CI check, and grow `apps/hyprspace` from there.

## Phase 2 results

Done on 2026-10-02 locally; the coordinator confirms the CI check on both OSes.

**What exists.** The workspace has the layout above, minus nothing:

- `crates/proto`: `Command`, `Event`, `RunEvent`, `SessionId`, the `Client` channel end, and the
  records the engine returns (git, agents, usage). No GPUI.
- `crates/harness`: the spike's one-run Claude stream-json adapter and `SESSION_ENV`. Phase 3
  turns it into the `Harness` trait.
- `crates/engine`: `Engine::start` and its command loop, plus copies of the Tauri Rust: `pty`,
  `hooks` (agenthook), `git/` (status, commit, setup, worktree), `providers`, `sessions` (resume
  list and resume mode), `skills`, `usage/local`, `usage/live` (the 180s Claude floor is now
  enforced here), `persist` (state in `~/.hyprspace/native`), and `env` (PATH rebuild, Claude
  session markers). What was left behind and why: [adr/0003](./adr/0003-copy-not-move-from-src-tauri.md).
- `crates/theme`: dark tokens and the 256-color terminal palette as plain data.
- `crates/ui`: `Root` (grid and event router), `transcript.rs`, `terminal/` (emulator, keys,
  paint, view). Depends on `proto` and `theme`, never on `engine`.
- `apps/hyprspace`: about 100 lines that start the engine, open the window and shut down on quit.
  `--chats` is now `--structured` (docs/CONTEXT.md).
- `docs/CONTEXT.md` for the vocabulary, ADRs 0002 (channel shape) and 0003 (copy, not move).
- `.github/workflows/check.yml`: fmt, clippy `-D warnings` and tests on `windows-latest` and
  `macos-latest`, toolchain 1.98.1, Metal toolchain fetched on macOS when the image lacks it.

**Checked.** 75 tests pass on Windows. `cargo clippy --target aarch64-apple-darwin` is clean for
the whole workspace when run with TLS turned off (aws-lc needs a mac C compiler) and a stub
`shaders.metallib` (built by `xcrun metal` on a mac), so the macOS code paths type-check. The app
ran with one structured and one terminal session: the reply streamed, keys posted to the window
reached claude's prompt, and closing it left no child processes.

**Not wired yet.** Only terminal and structured-session commands cross the channel. Git, usage,
skills, providers, resume list, hooks and persistence are tested library calls with no command;
each gets one when the UI first needs it.

