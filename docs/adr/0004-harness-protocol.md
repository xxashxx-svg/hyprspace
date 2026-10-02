# ADR 0004: One session handle per structured session, transcript events in proto

- Status: Accepted
- Date: 2026-10-02

## Context

Phase 3 needs a `Harness` trait with Claude (stream-json) and Codex (app-server) adapters that
start, send, steer, interrupt, answer approvals, resume, attach images, and pick model and
effort. The UI must render a structured transcript without ever seeing CLI JSON.

zeron's trait is `run(request, controls) -> Stream<AgentEvent>`: one call per prompt, with a
steering mailbox, an interrupt token and an input callback passed in. It also carries model
discovery, skills, titles, install and update checks, subagent routing and a 15-variant event
type. We need the protocol part, and our engine already has one command queue per direction.

## Decision

**The trait starts a session; the session takes the rest.**

```rust
trait Harness { fn agent(&self) -> Agent; fn start(&self, Launch, Emit) -> io::Result<Session>; }
Session::send(Prompt)            // starts a run, or steers the live one
Session::interrupt()             // ends the live run as Interrupted, session stays open
Session::answer(request, allow)  // answers a RunEvent::Approval
drop(session)                    // kills the CLI
```

Each adapter runs one tokio task per session that owns the child and its stdio. The engine maps
`Command::{OpenStructured, Send, Interrupt, Approve, Close}` onto these one to one, and `Emit`
wraps each `RunEvent` in `Event::Run` for the session's id.

**One `send`, not `send` and `steer`.** Whether a prompt steers depends on whether a run is
live, and only the harness knows that without a race: the UI's view of it lags by one event.
So a prompt sent mid-run steers, and one sent between runs starts a run. Each adapter falls back
on its own: a Codex `turn/steer` that loses the race with the turn's end is queued and starts
the next turn of the same run.

**Exactly one `Finished` per run.** Steering must not split a run in two:

- Claude: a `now` steer ends the turn it cuts into with a `result`. The harness writes each
  steer with a `uuid` and runs the CLI with `--replay-user-messages`; a `result` that arrives
  while a steer has not been echoed back is held until the next one. If the CLI never echoes
  it (the steer was absorbed into the turn that just ended), the held result is released after
  5 seconds of quiet. This is zeron's approach, minus its subagent bookkeeping.
- Claude steers go as `priority: "next"` while a tool call is open, because `now` aborts the
  tool; otherwise as `now`.
- Codex: a successful `turn/steer` joins the turn, so its one `turn/completed` ends the run.

**Interrupts give up after 5 seconds.** Claude gets an `interrupt` control request, Codex a
`turn/interrupt`. If the run has not ended by then, the CLI is killed and the session reports
`Failed`. zeron escalates SIGTERM then SIGKILL on a process group; we kill the child, which is
what Windows can do without job objects.

**Approvals surface, they are never auto-answered.** zeron auto-allows every tool (its sessions
run unattended). Ours ask: Claude's `can_use_tool` requests and Codex's
`item/commandExecution/requestApproval` and `item/fileChange/requestApproval` become
`RunEvent::Approval` with a typed `Tool`, and the CLI waits for `Command::Approve`. A file-change
request names only its item, so the Codex harness keeps tool items in flight to attach the
edits. Any other server request (user-input questions, MCP elicitations) gets a JSON-RPC error
back so the turn never hangs. Claude's `AskUserQuestion` arrives as an ordinary approval; a
question UI is phase 4's call.

**Permission modes, one enum for both CLIs:**

| `Permission` | Claude flag | Codex `approvalPolicy` / `sandbox` |
| --- | --- | --- |
| Plan | `--permission-mode plan` | `never` / `read-only` |
| Ask (default) | `--permission-mode default` | `on-request` / `read-only` |
| Auto | `--permission-mode acceptEdits` | `on-request` / `workspace-write` |
| Bypass | `--dangerously-skip-permissions` | `never` / `danger-full-access` |

The Codex rows follow its own presets (Read Only, Agent, Full Access), matching the Tauri app's
`codexCmd`. Codex has no plan mode, so Plan reads and never asks.

**Resume pins Claude's folder.** `claude --resume <id>` only finds a conversation from the folder
it started in. The Claude harness looks the id up under `~/.claude/projects/*/<id>.jsonl`
(or `CLAUDE_CONFIG_DIR`), reads the `cwd` its lines record, and spawns there whatever `cwd` the
caller passed. `Started.cwd` says where it ran. Codex threads carry their own folder, so
`thread/resume` gets no `cwd`; a thread that is gone fails the session instead of quietly
starting a new one, because the user asked for that conversation.

**Transcript events live in `proto::run`.** `RunEvent` carries `Started` (with the `thread` id
resume takes), `Text`, `Thinking`, `Tool` / `ToolDone`, `Approval`, `Steered`, `Usage`, `Error`,
`Finished { status, ms, text, error }` and `Failed`. `Tool` has a variant per thing the
transcript draws (command, read, edit with diffs, search, web, MCP, a subagent) and `Other` with
compact JSON for the rest. Edits carry unified-diff text: Codex sends real diffs; Claude's `Edit`, `MultiEdit`
and `Write` inputs become hunks with a bare `@@` because the CLI sends no line numbers. Tool
output is capped at 8 KB for display.

**Subagents report under their call.** Claude frames with a `parent_tool_use_id` belong to the
subagent that Agent call started. They come out as `SubagentTool`, `SubagentToolDone` and
`SubagentText` keyed by that call's id, never folded into the main reply (the bug zeron
documents), and the call's own `ToolDone` carries the subagent's report without the CLI's
hand-back framing. A background subagent answers its call at once with a launch note, so the
harness holds that call open until the subagent's `task_notification`, which can come after the
run's `Finished`. The CLI then takes a turn of its own to read the result; the harness reports it
as `Woke` followed by a normal run. Codex runs subagents as separate threads with their own
spawn, wait and close calls, so its notifications for another thread are still dropped.

**Left out on purpose.** Model discovery over the wire, skills, titles and the CLI install flow
stay out; the catalog is static plus Codex's own `models_cache.json`.

## Why not

- **zeron's `run(request, controls)` stream**: one call per prompt fits zeron's run journal. We
  have one live process per session and fire-and-forget commands, so a handle with three methods
  maps onto the engine without adapters in between.
- **Separate `steer` command**: see above; the UI cannot know a run is still live.
- **Auto-allowing tools like zeron**: the user already picks a permission mode, and Bypass is the
  way to skip the questions. Allowing everything under Ask would make the mode a lie.

## Consequences

- Tests drive each adapter through a Rust fake CLI (`crates/harness/fixtures/fake_cli`), built
  as a `fake-cli` binary of the harness crate so it runs on the Windows and macOS CI runners.
  A protocol change in a CLI shows up as a failing `examples/live.rs` run, not in CI.
- Until phase 4 draws approval prompts, the GPUI app denies every approval through the channel.
- The 5-second patience is a guess that held in zeron's live runs. Both adapters take it as
  `with_patience` so tests run at 300 ms.
- A Claude installed as an npm `.cmd` shim runs through `cmd /c`, and killing `cmd` can leave the
  node child behind. The native installer's `claude.exe` (what this machine has) is unaffected.
