# Structured sessions

A structured session drives an agent CLI over its machine protocol and draws a transcript. Code:
`crates/harness` (one adapter per agent), `proto::run` (the events the UI draws).

```rust
Harness::start(Launch, Emit) -> io::Result<Session>
Session::send(Prompt)            // starts a run, or steers the live one
Session::interrupt()             // ends the live run as Interrupted, the session stays open
Session::answer(request, Answer) // Allow, AllowAlways or Deny for a RunEvent::Approval
Session::configure(Launch)       // a new model, effort or permission, applied in place
drop(session)                    // kills the CLI
```

Each adapter runs one tokio task per session that owns the child and its stdio. The UI never sees
CLI JSON.

## Why terminal sessions stay first-class

Structured Claude runs the user's own `claude` headless. As of 2026-10-02, headless `claude -p`
draws on the plan's normal limits. Anthropic announced, then paused on 2026-06-15, a split that
would move headless, SDK and third-party use to a separate monthly credit at API rates, while
interactive Claude Code keeps the full plan. If that returns, a terminal session running `claude`
interactively is the way to keep the full plan. Sources:
[Anthropic support](https://support.claude.com/en/articles/15036540-use-the-claude-agent-sdk-with-your-claude-plan),
[Zed's summary](https://zed.dev/blog/anthropic-subscription-changes). Recheck them before changing
how Claude is driven.

## The two adapters

- **Claude** runs `claude --print --input-format stream-json --output-format stream-json --verbose
  --include-partial-messages --replay-user-messages --permission-prompt-tool stdio`. `--verbose` is
  required for stream-json output. `can_use_tool` control requests become approvals.
- **Codex** runs its app-server and talks JSON-RPC over stdio (`thread/start`, `turn/start`,
  `turn/steer`, `turn/interrupt`). Command and file-change approval requests become approvals. A
  file-change request names only its item, so the harness keeps tool items in flight to attach the
  edits. `item/tool/requestUserInput` becomes an `AskUserQuestion` approval and
  `turn/plan/updated` a `TodoWrite` call, so Codex gets Claude's question card and task list. A
  question with no options gets one that says the answer comes in a message. Any other server
  request gets a JSON-RPC error back so the turn never hangs.

## Rules that aren't obvious from the code

- **One `send`, not send and steer.** Only the harness knows without a race whether a run is live;
  the UI's view lags by one event. A Codex `turn/steer` that loses the race with the turn's end is
  queued and starts the next turn of the same run.
- **Exactly one `Finished` per run.** A Claude `now` steer cuts the current turn with a `result`.
  The harness writes each steer with a `uuid` and holds a `result` that arrives before the steer is
  echoed back, releasing it after 5 seconds of quiet if the CLI absorbed the steer. Steers go as
  `priority: "next"` while a tool call is open, because `now` aborts the tool.
- **Interrupts give up after 5 seconds.** Then the CLI is killed and the session reports `Failed`.
  Both adapters take the patience as `with_patience`, so tests run at 300 ms. That timer lives in
  the adapter's own loop, which can be stuck awaiting the CLI (Codex awaits `turn/start`), so the
  engine also ends a run still going 10 seconds after Stop (`engine/src/backstop.rs`).
- **Nothing writes to a CLI's stdin inline.** Lines go through `spawn::writer`, a task of their
  own. Claude writes hundreds of KB as it starts and reads stdin only after, so a session that
  awaited a big prompt (an image) stopped reading stdout and both sides waited on full pipes. The
  kill timer lives in the same loop, so Stop hung too.
- **"Chat about this" is a Deny that carries answers.** A plain Skip is a Deny with none. Claude
  gets its own clarify message with the answers so far, Codex gets a note as each answer. It
  needs no new `Answer` variant, which the phone would fail to read in the journal.
- **The CLI names its own commands and skills.** Claude answers the `initialize` request the
  harness sends first, and sends `system/commands_changed` after; plugin, claude.ai and MCP entries
  are only there. Codex answers `skills/list` and sends `skills/changed`. A skill runs wherever a
  message names it, a built-in or MCP prompt only when it starts one. The disk scan in
  `engine/src/skills.rs` covers a folder no session has opened yet.
- **Approvals are never auto-answered.** The user picks a permission mode, and Bypass is the way to
  skip the questions; allowing everything under Ask would make the mode a lie. The one answer
  given for the user is a deny: a message sent while a tool approval waits (from the box, the
  phone or the browser) denies it and steers the run, since queueing it behind a run that can't
  go on would never send it. Questions and plans are left alone. An approval that lands within
  1.5 s of a keystroke in the box stays off the dock until typing stops for that long, so an Enter
  already on its way can't deny something the user never saw.
- **A thread mentioned with `@` goes along as a file.** The engine writes its journal's prompts
  and replies to Markdown in `hyprspace-threads` in the temp folder, and the box attaches it as
  `[Thread N]`, so it works for a thread no view has opened. Claude starts with that folder as an
  `--add-dir`, or Ask first would stop to ask before reading a file outside the thread's folder.
- **Resume pins Claude's folder.** `claude --resume <id>` only finds a conversation from the folder
  it started in, so the harness reads the `cwd` recorded in `~/.claude/projects/*/<id>.jsonl` (or
  under `CLAUDE_CONFIG_DIR`) and spawns there, whatever the caller passed. Codex threads carry
  their own folder; one that's gone fails the session instead of quietly starting a new one.
- **A rewind happens at the next start.** "Edit from here" puts a `Rewind` (the prompt's text and
  how many earlier prompts said the same) on the thread's `Launch`, closes the CLI and trims the
  journal (`Command::TrimJournal`). Claude then resumes with its hidden `--resume-session-at`, at
  the `parentUuid` of that prompt in the session file, plus `--fork-session`, so the old file stays
  whole. Codex resumes, finds the prompt's turn with `thread/turns/list` and calls `thread/revert`.
  The prompt is found by text, not by an id in the journal, because the phone reads journal
  entries and an id there would change its wire.
- **A fork is a rewind into a new thread.** The new thread gets a copy of the journal up to the
  prompt (`Command::CopyJournal`) and a `Rewind` marked `fork`. Claude needs nothing more, since
  every rewind already forks. Codex's `thread/revert` rewrites the thread it is given, so a fork
  opens with `thread/fork` and reverts the copy. A Claude fork whose prompt isn't found still
  passes `--fork-session`, or two threads would write one conversation.
- **Slash commands are ours first.** The CLI's machine mode answers commands that only exist
  interactively with "isn't available in this environment", and some it does take (`/fast`,
  `/rename`) change the CLI without telling HyprSpace. So `transcript/commands.rs` handles the
  ones HyprSpace can do itself before anything reaches the CLI, names the rest of the
  interactive-only ones as terminal-only, and passes everything else through. The interactive
  list comes from the CLI binary, the machine one from its `initialize` answer; check both when
  Claude Code adds commands. `/branch` is a fork with an empty `Rewind`, which keeps the whole
  conversation.
- **`/btw` is a fork that answers once.** The CLI's machine mode has no `/btw`, so a side question
  runs a second `claude` with the thread's own flags plus `--fork-session
  --no-session-persistence --max-turns 1`, and its answer stays out of the journal. Keeping the
  flags keeps the prompt cache: `--tools ""` or plan mode would make it read the whole conversation
  again at full price. Permission drops to Ask, which the cache doesn't see, and every tool request
  is refused. It sees the conversation up to the last finished reply, since that's what Claude has
  saved. Claude threads on this computer only.
- **A thread names itself with one quiet run.** A new thread's first message, or the whole
  conversation for Regenerate title, goes on stdin to `claude -p` on Haiku or `codex exec
  --ephemeral` on the catalog's Luna model. Nothing is saved, so it never shows in either CLI's
  resume list. Claude's run has no tools or MCP servers and a `--settings` file with
  `disableAllHooks`, or the user's own Stop hooks would fire on every new thread; `--bare` can't do
  this, since it drops the subscription login. The name lands only if the thread still has the
  title it had when it asked, so a rename in between wins. On by default, with a switch in
  Settings, General.
- **Changes apply in place.** Claude takes a model, 1M window included, as `set_model`, a
  permission as `set_permission_mode`, and effort and fast mode as `apply_flag_settings`. Every
  launch passes `--allow-dangerously-skip-permissions`, or the CLI refuses a later switch to
  Bypass. Fast mode goes off as `false`, because `null` leaves the launch's `--settings` file in
  force. A change the CLI refuses shows as an error in the thread. Codex sends `effort` on every
  `turn/start`, and `model`, `approvalPolicy` and `sandboxPolicy` once they differ from what the
  thread opened with.
- **Subagents report under their call.** Claude frames with a `parent_tool_use_id` become
  `SubagentTool`, `SubagentToolDone` and `SubagentText` keyed by the Agent call, never folded into
  the main reply. A background subagent can finish after the run; the CLI then takes a turn of its
  own, reported as `Woke` followed by a normal run.
- **Background tasks are a live list.** Claude's `background_tasks_changed` carries every task
  still running, so `RunEvent::Background` replaces the list each time, and the CLI's `ambient`
  watchers are left out. An open task asks the CLI for the end of its output (`get_task_output`)
  once a second, and only while it's open. The CLI refuses that for anything but a command or a
  monitor, so an open subagent shows its last calls from its card in the thread. None of it is journaled: the tasks die with the CLI,
  so a phone or another computer never sees them. Codex has no such list yet (`thread/backgroundTerminals/list` is a
  request, not an event).
- **A reply's links and pictures stay on this computer.** A link, or a path written as code, that
  names a file in the thread's folder opens in the viewer at its line. A picture on its own line
  draws only when it is a local file: one on the web is never fetched, because the app makes no
  request the user didn't ask for. A thread on another computer opens no files.
- **Edits carry unified diffs.** Codex sends real ones. Claude sends only the old and new text,
  so the harness reads the file and finds the old text, or the new text when the CLI has already
  made the edit, for line numbers and 3 lines of context. A text it can't find, and every
  `Write`, gets a hunk with a bare `@@`. Tool output is capped at 8 KB for display.
- **A thread can switch between structured and terminal.** Both kinds keep the same `Launch`, so
  the other side resumes the same conversation. The old CLI is closed first, and a busy thread
  can't switch. A structured side opens at once with no prompt, because a bad `--resume` only
  fails when the CLI starts, and counts as up if it hasn't failed after 4 seconds. A terminal
  side counts as up once Claude's SessionStart hook names the conversation, or once Codex shows
  up under the shell. If the new side fails, quits before it is up, or a terminal isn't up
  within 20 seconds, the thread goes back to what it was. Until then the menu shows how long is
  left. Messages sent in
  the terminal aren't in our journal. For Claude, the switch appends what Claude saved past the
  journal's last prompt, counted by prompts, read from its session file the same way the live
  stream is. For Codex the transcript says the messages aren't shown.
- **A message the harness doesn't know is named in `protocol.log`** in the state folder, once per
  name while the app runs, with no content. Read it after a Claude Code or Codex update. The
  known lists in `crates/harness/src/protocol.rs` come from the Agent SDK's `sdk.d.ts` for the
  matching `claudeCodeVersion` in its `package.json`, and from `codex app-server
  generate-json-schema`.
- **A `.cmd` shim runs through `cmd /c`.** Killing `cmd` can leave the node child behind. The
  native `claude.exe` is unaffected.

## Permission modes

One enum for both CLIs:

| `Permission` | Claude | Codex `approvalPolicy` / `sandbox` |
| --- | --- | --- |
| Plan | `--permission-mode plan` | `never` / `read-only` |
| Ask (default) | `--permission-mode default` | `on-request` / `read-only` |
| Auto | `--permission-mode acceptEdits` | `on-request` / `workspace-write` |
| Bypass | `--dangerously-skip-permissions` | `never` / `danger-full-access` |

The Codex rows follow its own presets. Codex has no plan mode, so Plan reads and never asks.

## Queue, limits and delegation

- **A message sent mid-run waits.** It goes in the thread's queue (`Thread.queue`, saved with the
  state) and sends when the run finishes as Done. Steer sends one now, into the live run. A stopped
  run and a restart hold the queue until the user sends.
- **A usage limit continues by itself.** Claude's rejected `rate_limit_event` (with `resetsAt`) or
  its `rate_limit` error, and Codex's `usageLimitExceeded`, become `RunEvent::Limited`. The UI saves
  `Thread.resume_at`: the CLI's reset time, else the usage meter's spent window, else 30 minutes,
  plus a minute. When it comes, the thread gets a continue message. Any message sent before that
  cancels it.
- **An update picks cut-off runs back up; nothing else does.** When an install starts, every
  thread mid-run saves `Thread.updating` with its background tasks' labels. The first launch of the
  new version opens those threads, and one whose journal shows the run was cut off gets a continue
  message naming the background work that stopped, since the CLI never reports on it again. A
  crash or a plain close leaves the Resume button. A cut-off run that nothing came back for sends
  its prompt again instead: the journal saves a prompt before the CLI takes it, so the CLI may
  never have seen it, and a continue message would find nothing to continue. A copy with `HYPRSPACE_DEV` or
  `HYPRSPACE_STATE_DIR` never picks up: it runs on copies of real threads, and would start real
  agents on them.
- **Delegation is an MCP tool.** A top-level structured session gets the `hyprspace` MCP server:
  our own binary as `hyprspace mcp <port> <token>`, passed to Claude as an `--mcp-config` file and
  to Codex as `config` on `thread/start`. Its one tool, `delegate`, posts to a loopback listener
  (`engine/src/delegate.rs`), which hands `Event::Delegate` to the UI and holds the call open. The
  UI starts a child thread (`Thread.parent`) with the parent's folder and permission, and answers
  with `Command::Delegated` and the child's last reply when it finishes. Stopping the parent stops
  its children. Children get no tool, so delegation is one level deep. Codex asks about MCP calls
  with `mcpServer/elicitation/request`, which shows as a normal approval.

## Journals

Each structured thread appends every prompt, answer and run event to
`journals/thread-<id>.jsonl`, joining streamed text into one line per reply. After a restart the
transcript replays it through the same calls it uses for live events. The CLI's thread id lands in
the thread's `Launch.resume` when `Started` arrives, so the next prompt reopens the conversation.

- We keep our own journal instead of reading the CLIs' transcripts: theirs differ in shape and
  change between versions, and ours is the transcript as we drew it.
- A crash loses only the reply that was still streaming. Journals aren't trimmed yet, and removing
  a thread leaves its journal.
- A new `RunEvent` variant stays out of the journal unless the phone knows it. Phones read
  journal entries in batches, and one entry the Android app can't decode drops the batch. A new
  field is safe when it is left out while empty, as `ToolDone.images` is.

## Testing

Both adapters run against `crates/harness/fixtures/fake_cli`, a Rust fake of both CLIs built as a
bin of the harness crate so it runs on both CI runners. A protocol change in a real CLI shows up in
`examples/live.rs`, which runs one real session, not in CI.
