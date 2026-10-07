# Structured sessions

A structured session drives an agent CLI over its machine protocol and draws a transcript. Code:
`crates/harness` (one adapter per agent), `proto::run` (the events the UI draws).

```rust
Harness::start(Launch, Emit) -> io::Result<Session>
Session::send(Prompt)            // starts a run, or steers the live one
Session::interrupt()             // ends the live run as Interrupted, the session stays open
Session::answer(request, Answer) // Allow, AllowAlways or Deny for a RunEvent::Approval
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
  edits. Any other server request gets a JSON-RPC error back so the turn never hangs.

## Rules that aren't obvious from the code

- **One `send`, not send and steer.** Only the harness knows without a race whether a run is live;
  the UI's view lags by one event. A Codex `turn/steer` that loses the race with the turn's end is
  queued and starts the next turn of the same run.
- **Exactly one `Finished` per run.** A Claude `now` steer cuts the current turn with a `result`.
  The harness writes each steer with a `uuid` and holds a `result` that arrives before the steer is
  echoed back, releasing it after 5 seconds of quiet if the CLI absorbed the steer. Steers go as
  `priority: "next"` while a tool call is open, because `now` aborts the tool.
- **Interrupts give up after 5 seconds.** Then the CLI is killed and the session reports `Failed`.
  Both adapters take the patience as `with_patience`, so tests run at 300 ms.
- **Approvals are never auto-answered.** The user picks a permission mode, and Bypass is the way to
  skip the questions; allowing everything under Ask would make the mode a lie.
- **Resume pins Claude's folder.** `claude --resume <id>` only finds a conversation from the folder
  it started in, so the harness reads the `cwd` recorded in `~/.claude/projects/*/<id>.jsonl` (or
  under `CLAUDE_CONFIG_DIR`) and spawns there, whatever the caller passed. Codex threads carry
  their own folder; one that's gone fails the session instead of quietly starting a new one.
- **Subagents report under their call.** Claude frames with a `parent_tool_use_id` become
  `SubagentTool`, `SubagentToolDone` and `SubagentText` keyed by the Agent call, never folded into
  the main reply. A background subagent can finish after the run; the CLI then takes a turn of its
  own, reported as `Woke` followed by a normal run.
- **Edits carry unified diffs.** Codex sends real ones. Claude's `Edit`, `MultiEdit` and `Write`
  inputs become hunks with a bare `@@`, because the CLI sends no line numbers. Tool output is
  capped at 8 KB for display.
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

## Journals

Each structured thread appends every prompt, answer and run event to
`journals/thread-<id>.jsonl`, joining streamed text into one line per reply. After a restart the
transcript replays it through the same calls it uses for live events. The CLI's thread id lands in
the thread's `Launch.resume` when `Started` arrives, so the next prompt reopens the conversation.

- We keep our own journal instead of reading the CLIs' transcripts: theirs differ in shape and
  change between versions, and ours is the transcript as we drew it.
- A crash loses only the reply that was still streaming. Journals aren't trimmed yet, and removing
  a thread leaves its journal.

## Testing

Both adapters run against `crates/harness/fixtures/fake_cli`, a Rust fake of both CLIs built as a
bin of the harness crate so it runs on both CI runners. A protocol change in a real CLI shows up in
`examples/live.rs`, which runs one real session, not in CI.
