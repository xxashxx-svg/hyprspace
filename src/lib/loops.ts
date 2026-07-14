// The loop engine: drives a LoopDef per its mode (interval / until-done / cron / manual), runs each
// iteration through the headless agent runner (agent.rs), and enforces the safety guards. App-open
// lifetime — controllers live in a module map and are torn down on stop.
import {
  useLoops,
  loopRunId,
  type LoopDef,
  type LoopEvent,
  type LoopStatus,
  type ScheduleCfg,
} from "../stores/loops";
import { useProjectConfigs } from "../stores/projectConfig";
import { useNotifications } from "../stores/notifications";
import { isWindows } from "../platform";
import {
  agentStart,
  agentStop,
  runCheck,
  worktreeCreate,
  gitIsRepo,
  gitChanges,
  revealPath,
  secretHas,
  prepareNotifySettings,
  cleanupHookRun,
  readFile,
} from "../api";
import { nextCron, cronValid } from "./cron";
import { startLoopTerm, stopLoopTerm } from "../terminal/loopTerm";

const EXIT = "\0__agent_exit__"; // matches the sentinel agent.rs emits when a turn ends
const NOPROGRESS_LIMIT = 3; // consecutive unchanged/empty iterations → crash-loop stop
const UNTIL_DELAY_MS = 1200; // breather between until-done iterations
const FAIL_LIMIT = 3; // consecutive failed iterations → stop + alert instead of burning quota
const RETRY_BASE_MS = 5000; // failed-iteration backoff: 5s, 10s, 20s

// setTimeout wraps at 2^31-1 ms (~24.8 days) and fires IMMEDIATELY on overflow — a monthly cron
// would run back-to-back until max iterations. chain shorter sleeps for far-off fires.
const MAX_TIMEOUT_MS = 2_147_000_000;
function armAt(ctrl: Ctrl, at: number, fn: () => void) {
  const delay = at - Date.now();
  if (delay > MAX_TIMEOUT_MS) ctrl.timer = setTimeout(() => armAt(ctrl, at, fn), MAX_TIMEOUT_MS);
  else ctrl.timer = setTimeout(fn, Math.max(1000, delay));
}

// pre-run git state per loop id, so the end-of-run diff only counts what THIS run changed —
// not pre-existing uncommitted edits or a reused worktree's leftovers
const baselines = new Map<string, Map<string, { a: number; r: number }>>();

type Ctrl = {
  stop: boolean;
  paused: boolean;
  timer?: ReturnType<typeof setTimeout>;
  pollTimer?: ReturnType<typeof setInterval>; // interactive-loop notify-marker poll
};
const ctrls = new Map<string, Ctrl>();
// clear the controller's timers (tick + the interactive-loop poll)
function clearTimers(ctrl: Ctrl) {
  if (ctrl.timer) clearTimeout(ctrl.timer);
  if (ctrl.pollTimer) clearInterval(ctrl.pollTimer);
}

export const isLoopActive = (id: string) => ctrls.has(id);

// permission/sandbox flags differ per CLI — map our four UI modes onto each one's vocabulary.
const claudePerm: Record<string, string> = {
  plan: "plan",
  acceptEdits: "acceptEdits",
  bypass: "bypassPermissions",
  default: "default",
};
const codexSandbox: Record<string, string> = {
  plan: "read-only",
  acceptEdits: "workspace-write",
  default: "workspace-write", // headless can't "ask", so give it room to actually work
};
const geminiApproval: Record<string, string> = {
  plan: "plan",
  acceptEdits: "auto_edit",
  bypass: "yolo",
  default: "default",
};

// the provider argv for one iteration. The prompt always arrives on stdin (see agent.rs), so argv
// only flips the CLI into headless mode + sets model/permissions. `cont` = continue the prior
// session (only on iterations after the first, when session mode is "continue").
// both Claude backends are headless `claude -p`; they differ only in auth (see secretsFor):
// "claude" → an Anthropic API key, "claude-sub" → the logged-in subscription.
function isClaudeStream(p: string): boolean {
  return p === "claude" || p === "claude-sub" || p === "claude-hooks";
}

// backends whose CLI can run an interactive session seeded with the task (pane run mode)
export function paneCapable(p: string): boolean {
  return isClaudeStream(p) || p === "codex" || p === "gemini";
}

function buildArgs(def: LoopDef, cont: boolean): string[] {
  const pm = def.permissionMode || "acceptEdits";
  if (isClaudeStream(def.provider)) {
    // headless `claude -p`. stream-json (requires --verbose) emits one event per step so the Runs
    // tab shows live structured progress. NO --bare: that flag disables the subscription OAuth login.
    const a = ["claude", "-p", "--output-format", "stream-json", "--verbose"];
    if (def.model) a.push("--model", def.model);
    a.push("--permission-mode", claudePerm[pm] ?? "acceptEdits");
    if (cont) a.push("--continue"); // resume the most recent conversation in this folder
    return a;
  }
  if (def.provider === "codex") {
    // codex headless — uses your `codex login` auth. fresh `exec` reads the prompt from stdin;
    // `resume --last` continues the last session, the trailing `-` makes it read stdin.
    // --json gives structured events (agent text, commands, token usage) for the live transcript.
    // resume must carry the SAME model/sandbox flags — codex doesn't remember them per session,
    // so dropping them silently reverts iteration 2+ to the user's config defaults.
    const a = cont ? ["codex", "exec", "resume", "--last", "--json"] : ["codex", "exec", "--json"];
    if (def.model) a.push("-m", def.model);
    if (pm === "bypass") a.push("--dangerously-bypass-approvals-and-sandbox");
    else a.push("-s", codexSandbox[pm] ?? "workspace-write");
    a.push("--skip-git-repo-check");
    if (cont) a.push("-");
    return a;
  }
  if (def.provider === "opencode") {
    // opencode headless: `run` reads the prompt on stdin (agent.rs feeds it). uses opencode's own
    // configured provider/model + auth; -m sets a "provider/model"; --continue keeps one session.
    const a = ["opencode", "run"];
    if (def.model) a.push("-m", def.model);
    if (pm === "bypass") a.push("--dangerously-skip-permissions");
    if (cont) a.push("--continue");
    return a;
  }
  if (def.provider === "grok") {
    // grok (xAI) headless takes the prompt as an ARGV (`grok -p …`) — unlike the others it doesn't
    // read stdin — and emits plain text we handle like codex/gemini. runs on grok's own login
    // (browser token in ~/.grok or XAI_API_KEY); --always-approve so headless isn't stuck on a prompt.
    const a = ["grok", "-p", def.prompt];
    if (def.model) a.push("-m", def.model);
    if (pm !== "plan") a.push("--always-approve");
    if (cont) a.push("-c");
    return a;
  }
  // gemini — `-p` flips to headless; empty value so only the stdin prompt counts. best-effort.
  const a = ["gemini"];
  if (def.model) a.push("-m", def.model);
  a.push("-p", "", "--approval-mode", geminiApproval[pm] ?? "default", "--skip-trust");
  if (cont) a.push("--resume", "latest");
  return a;
}

// keychain secrets injected as env for this backend. Only "claude" (the API-key backend) injects a
// key; "claude-sub" runs on the logged-in subscription and Codex/Gemini use their own CLI login, so
// none of them get a key — `claude -p` then authenticates exactly like the terminal panes do.
function secretsFor(def: LoopDef): Record<string, string> {
  return def.provider === "claude" ? { ANTHROPIC_API_KEY: "anthropic" } : {};
}

// short label for a tool call, e.g. "Edit README.md" or "Bash: npm test"
function toolLabel(name: string, input: Record<string, unknown> | undefined): string {
  const path = (input?.file_path ?? input?.path) as string | undefined;
  if (name === "Bash") {
    // claude usually prefixes `cd "<worktree>" && <real cmd>` — drop it so the real command shows
    const cmd = String(input?.command ?? "").trim().replace(/^cd\s+("[^"]*"|'[^']*'|\S+)\s*&&\s*/, "");
    return `Bash: ${cmd.slice(0, 90)}`;
  }
  if (path) return `${name} ${path.split(/[\\/]/).pop()}`;
  return name;
}

// the argument we surface for a tool call: the command for Bash, the path for file tools, the
// pattern for search tools. left full here — the transcript truncates long ones itself.
function toolArg(name: string, input: Record<string, unknown> | undefined): string {
  if (name === "Bash") {
    return String(input?.command ?? "").trim().replace(/^cd\s+("[^"]*"|'[^']*'|\S+)\s*&&\s*/, "");
  }
  const path = (input?.file_path ?? input?.path) as string | undefined;
  if (path) return path;
  const pat = (input?.pattern ?? input?.query) as string | undefined;
  return pat ?? "";
}

let evSeq = 0;
function newLoopEvent(iter: number, partial: Partial<LoopEvent>): LoopEvent {
  return { id: `e${Date.now().toString(36)}-${evSeq++}`, iteration: iter, ts: Date.now(), kind: "text", ...partial };
}

// parse one claude stream-json line into transcript events: a tool_use becomes a "running" tool row,
// its tool_result flips it to ok/error with a duration, and thinking/text/result carry the words.
// `append` keeps the flat log line going for the classic panel. non-JSON (stderr) passes through.
// pull a token count out of the stream's usage block (input + output + cache, best-effort)
function usageTokens(usage: Record<string, unknown> | undefined): number {
  if (!usage) return 0;
  const n = (k: string) => (typeof usage[k] === "number" ? (usage[k] as number) : 0);
  return n("input_tokens") + n("output_tokens") + n("cache_creation_input_tokens") + n("cache_read_input_tokens");
}

function handleClaudeLine(
  loopId: string,
  iter: number,
  line: string,
  toolMap: Map<string, { eventId: string; start: number }>,
  setResult: (r: string, tokens: number, cost: number, isError: boolean) => void,
  append: (line: string) => void,
) {
  const S = useLoops.getState;
  let ev: Record<string, unknown>;
  try {
    ev = JSON.parse(line);
  } catch {
    S().pushEvent(loopId, newLoopEvent(iter, { kind: "text", text: line }));
    append(line);
    return;
  }
  if (ev.type === "assistant") {
    const content = (ev.message as { content?: unknown[] } | undefined)?.content ?? [];
    for (const b of content as Array<Record<string, unknown>>) {
      if (b.type === "text" && typeof b.text === "string" && b.text.trim()) {
        const t = b.text.trim();
        S().pushEvent(loopId, newLoopEvent(iter, { kind: "text", text: t }));
        append(t);
      } else if (b.type === "thinking" && typeof b.thinking === "string" && b.thinking.trim()) {
        S().pushEvent(loopId, newLoopEvent(iter, { kind: "thinking", text: b.thinking.trim() }));
      } else if (b.type === "tool_use") {
        const name = b.name as string;
        const input = b.input as Record<string, unknown>;
        const e = newLoopEvent(iter, { kind: "tool", tool: name, arg: toolArg(name, input), status: "running" });
        if (typeof b.id === "string") toolMap.set(b.id, { eventId: e.id, start: e.ts });
        S().pushEvent(loopId, e);
        append(`→ ${toolLabel(name, input)}`);
      }
    }
    return;
  }
  if (ev.type === "user") {
    const content = (ev.message as { content?: unknown[] } | undefined)?.content ?? [];
    for (const b of content as Array<Record<string, unknown>>) {
      if (b.type === "tool_result" && typeof b.tool_use_id === "string") {
        const t = toolMap.get(b.tool_use_id);
        if (t) {
          S().patchEvent(loopId, t.eventId, { status: b.is_error ? "error" : "ok", durationMs: Date.now() - t.start });
          toolMap.delete(b.tool_use_id);
        }
      }
    }
    return;
  }
  if (ev.type === "result") {
    const r = typeof ev.result === "string" ? ev.result : "";
    const costNum = typeof ev.total_cost_usd === "number" ? (ev.total_cost_usd as number) : 0;
    const tokens = usageTokens(ev.usage as Record<string, unknown> | undefined);
    // the turn finished — any tool still showing "running" did complete, so settle it
    for (const t of toolMap.values()) S().patchEvent(loopId, t.eventId, { status: "ok" });
    toolMap.clear();
    const costStr = costNum ? ` · $${costNum.toFixed(3)}` : "";
    setResult(r, tokens, costNum, ev.is_error === true);
    S().pushEvent(loopId, newLoopEvent(iter, { kind: "result", text: r, arg: costStr.trim() || undefined }));
    append(`✓ done${costStr}`);
  }
}

// parse one `codex exec --json` line: agent_message → text (and the iteration's result), reasoning
// → thinking, command_execution → a tool row, turn.completed carries token usage, turn.failed marks
// the iteration failed. non-JSON lines (stderr noise) pass through as plain text.
function handleCodexLine(
  loopId: string,
  iter: number,
  line: string,
  state: { result: string; tokens: number; failed: boolean },
  itemMap: Map<string, { eventId: string; start: number }>,
  append: (line: string) => void,
) {
  const S = useLoops.getState;
  let ev: Record<string, unknown>;
  try {
    ev = JSON.parse(line);
  } catch {
    S().pushEvent(loopId, newLoopEvent(iter, { kind: "text", text: line }));
    append(line);
    return;
  }
  const item = ev.item as Record<string, unknown> | undefined;
  if (ev.type === "item.started" && item?.type === "command_execution") {
    const cmd = String(item.command ?? "").slice(0, 200);
    const e = newLoopEvent(iter, { kind: "tool", tool: "Bash", arg: cmd, status: "running" });
    if (typeof item.id === "string") itemMap.set(item.id, { eventId: e.id, start: e.ts });
    S().pushEvent(loopId, e);
    append(`→ Bash: ${cmd.slice(0, 90)}`);
    return;
  }
  if (ev.type === "item.completed" && item) {
    if (item.type === "agent_message" && typeof item.text === "string" && item.text.trim()) {
      state.result = item.text.trim();
      S().pushEvent(loopId, newLoopEvent(iter, { kind: "text", text: state.result }));
      append(state.result);
    } else if (item.type === "reasoning" && typeof item.text === "string" && item.text.trim()) {
      S().pushEvent(loopId, newLoopEvent(iter, { kind: "thinking", text: item.text.trim() }));
    } else if (item.type === "command_execution") {
      const failed = item.exit_code !== undefined && item.exit_code !== 0;
      const t = typeof item.id === "string" ? itemMap.get(item.id) : undefined;
      if (t) {
        S().patchEvent(loopId, t.eventId, { status: failed ? "error" : "ok", durationMs: Date.now() - t.start });
        if (typeof item.id === "string") itemMap.delete(item.id);
      } else {
        const cmd = String(item.command ?? "").slice(0, 200);
        S().pushEvent(loopId, newLoopEvent(iter, { kind: "tool", tool: "Bash", arg: cmd, status: failed ? "error" : "ok" }));
        append(`→ Bash: ${cmd.slice(0, 90)}`);
      }
    } else if (item.type === "error" && typeof item.message === "string") {
      S().pushEvent(loopId, newLoopEvent(iter, { kind: "text", text: `⚠ ${item.message}` }));
      append(`⚠ ${item.message}`);
    }
    return;
  }
  if (ev.type === "turn.completed") {
    const u = ev.usage as Record<string, unknown> | undefined;
    const n = (k: string) => (typeof u?.[k] === "number" ? (u[k] as number) : 0);
    state.tokens += n("input_tokens") + n("cached_input_tokens") + n("output_tokens");
    for (const t of itemMap.values()) S().patchEvent(loopId, t.eventId, { status: "ok" });
    itemMap.clear();
    if (state.result) {
      S().pushEvent(loopId, newLoopEvent(iter, { kind: "result", text: state.result }));
      append("✓ done");
    }
    return;
  }
  if (ev.type === "turn.failed") {
    state.failed = true;
    const msg = String((ev.error as Record<string, unknown>)?.message ?? "turn failed");
    S().pushEvent(loopId, newLoopEvent(iter, { kind: "text", text: `⚠ ${msg}` }));
    append(`⚠ ${msg}`);
  }
  // thread.started / turn.started are noise — skip
}

function hash(s: string): string {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) % 2147483647;
  return String(h);
}
function firstLine(s: string): string {
  const l = s.split("\n").find((x) => x.trim());
  return (l ?? "").trim().slice(0, 160);
}

// next fire time for a cron-mode loop: a real 5-field cron expression wins, then the simple forms.
function nextFire(s?: ScheduleCfg): number {
  const now = Date.now();
  if (s?.cron && cronValid(s.cron)) {
    const n = nextCron(s.cron, now);
    if (n) return n;
  }
  if (s?.everyMin && s.everyMin > 0) return now + s.everyMin * 60000;
  if (s?.dailyAt && /^\d{1,2}:\d{2}$/.test(s.dailyAt)) {
    const [h, m] = s.dailyAt.split(":").map(Number);
    const d = new Date();
    d.setHours(h, m, 0, 0);
    if (d.getTime() <= now) d.setDate(d.getDate() + 1);
    return d.getTime();
  }
  return now + 60 * 60000; // default hourly
}

// run ONE iteration headless, streaming lines to `append`; resolves with the full output once the
// turn's exit sentinel arrives. `cont` continues the prior session instead of starting fresh.
interface IterResult {
  out: string; // readable output, used for progress/sentinel checks
  tokens: number; // tokens this iteration burned (claude + codex; 0 otherwise)
  cost: number; // USD this iteration cost (claude only)
  failed: boolean; // the CLI failed to start or the turn errored — drives the retry policy
}

function runIteration(def: LoopDef, folder: string, cont: boolean, append: (line: string) => void): Promise<IterResult> {
  return new Promise((resolve) => {
    const rid = loopRunId(def.id);
    const loopId = def.id;
    const iter = useLoops.getState().runs[loopId]?.iteration ?? 0;
    const env = useProjectConfigs.getState().getConfig(def.folder).env;
    const lines: string[] = []; // readable text, used for progress/sentinel checks
    const toolMap = new Map<string, { eventId: string; start: number }>(); // tool_use id → its event + start
    const codex = { result: "", tokens: 0, failed: false }; // per-iteration codex --json state
    let result = ""; // claude's final result text (when stream-json reports it)
    let tokens = 0;
    let cost = 0;
    let failed = false;
    let done = false;
    const record = (line: string) => {
      lines.push(line);
      append(line);
    };
    const finish = () => {
      if (done) return;
      done = true;
      resolve({
        out: result || codex.result || lines.join("\n"),
        tokens: tokens + codex.tokens,
        cost,
        failed: failed || codex.failed,
      });
    };
    void agentStart(rid, folder, buildArgs(def, cont), env, secretsFor(def), def.prompt, (line) => {
      if (line === EXIT) {
        finish();
        return;
      }
      // claude + codex stream JSON events → structured transcript; others are plain text
      if (isClaudeStream(def.provider)) {
        handleClaudeLine(loopId, iter, line, toolMap, (r, t, c, err) => {
          result = r;
          tokens = t;
          cost = c;
          if (err) failed = true;
        }, record);
      } else if (def.provider === "codex") {
        handleCodexLine(loopId, iter, line, codex, toolMap, record);
      } else {
        useLoops.getState().pushEvent(loopId, newLoopEvent(iter, { kind: "text", text: line }));
        record(line);
      }
    }).catch((e) => {
      record(`⚠ failed to start: ${e}`);
      failed = true;
      finish();
    });
  });
}

// a run just ended: compute what it changed, write the persisted history entry, and (unless the
// user stopped it themselves) raise a notification with the outcome. `lastResult` is the final
// agent output captured BEFORE finish() overwrote it with the stop note.
async function recordRunEnd(
  id: string,
  status: LoopStatus,
  note: string | undefined,
  notify: boolean,
  lastResult?: string,
) {
  const S = useLoops.getState;
  const def = S().loops[id];
  const run = S().runs[id];
  // no lastRunAt = the run never actually did work (an armed cron waiting for its first fire) —
  // recording "0 iterations, stopped" would just be noise
  if (!def || !run?.startedAt || !run.lastRunAt) return;
  const name = def.name || "Automation";
  const gen = run.startedAt; // generation marker — a restart resets startedAt via resetRun

  // grab the baseline before the first await, so a restarted run's fresh baseline is never consumed
  const base = baselines.get(id) ?? new Map<string, { a: number; r: number }>();
  baselines.delete(id);

  // diff summary — the worktree if isolated, else the folder itself, as a DELTA against the
  // baseline so pre-existing edits (or a reused worktree's leftovers) aren't blamed on this run
  let filesChanged: number | undefined;
  let additions: number | undefined;
  let deletions: number | undefined;
  try {
    const dir = run.worktreePath || def.folder;
    if (dir) {
      const ch = await gitChanges(dir);
      const changed = ch.filter((c) => {
        const b = base.get(c.path);
        return !b || b.a !== c.added || b.r !== c.removed;
      });
      if (changed.length) {
        filesChanged = changed.length;
        additions = changed.reduce((n, c) => n + Math.max(0, c.added - (base.get(c.path)?.a ?? 0)), 0);
        deletions = changed.reduce((n, c) => n + Math.max(0, c.removed - (base.get(c.path)?.r ?? 0)), 0);
      }
    }
  } catch {
    /* not a repo / folder gone — no diff to report */
  }

  // we awaited — the loop may have been deleted or restarted since. Never resurrect a removed
  // run's state, and never stamp the OLD run's numbers onto a freshly started run.
  const still = useLoops.getState();
  if (!still.loops[id]) return; // deleted while we diffed — drop everything
  const sameRun = still.runs[id]?.startedAt === gen;
  if (filesChanged && sameRun) {
    still.setRun(id, { filesChanged, additions, deletions });
    still.appendLog(id, `${filesChanged} file${filesChanged === 1 ? "" : "s"} changed · +${additions} −${deletions}`);
  }

  still.addHistory(id, {
    id: crypto.randomUUID(),
    startedAt: run.startedAt,
    endedAt: Date.now(),
    status,
    iterations: run.iteration,
    note,
    lastResult: lastResult ?? run.lastResult,
    tokensUsed: run.tokensUsed,
    costUsed: run.costUsed,
    worktreePath: run.worktreePath,
    filesChanged,
    additions,
    deletions,
  });

  if (notify) {
    const title =
      status === "done"
        ? `${name} finished`
        : status === "crashloop"
          ? `${name} stopped — no progress`
          : status === "error"
            ? `${name} failed`
            : `${name} stopped`;
    const diff = filesChanged ? `${filesChanged} file${filesChanged === 1 ? "" : "s"} · +${additions} −${deletions}` : "";
    const body = [note, diff, run.costUsed ? `$${run.costUsed.toFixed(2)}` : ""].filter(Boolean).join(" · ");
    useNotifications.getState().add({ title, body: body || undefined, kind: "info" });
  }
}

export function startLoop(id: string) {
  if (ctrls.has(id)) return; // already running
  const def0 = useLoops.getState().loops[id];
  if (!def0) return;

  const ctrl: Ctrl = { stop: false, paused: false };
  ctrls.set(id, ctrl);
  const S = () => useLoops.getState();
  S().resetRun(id);
  S().setRun(id, { status: "running", startedAt: Date.now(), iteration: 0, stale: 0 });
  const append = (line: string) => S().appendLog(id, line);
  // iteration output only — swallowed once the run is over, so late buffered lines from a killed
  // agent can't pollute a stopped run's log
  const streamAppend = (line: string) => {
    if (!ctrl.stop) append(line);
  };
  let lastHash = "";
  let fails = 0; // consecutive failed iterations (drives retry backoff + the fail-stop)
  let finished = false; // reentry guard — a PTY exit event can race the stop path
  let runFolder = def0.folder; // where iterations actually run — a worktree if isolation is on

  const finish = (status: LoopStatus, note?: string) => {
    if (finished) return;
    finished = true;
    ctrl.stop = true;
    clearTimers(ctrl);
    ctrls.delete(id);
    void agentStop(loopRunId(id)).catch(() => {});
    stopLoopTerm(loopRunId(id)); // tears down an interactive-loop PTY; no-op for headless loops
    void cleanupHookRun(loopRunId(id)).catch(() => {}); // drop the run's temp hook/notify files
    const prevResult = S().runs[id]?.lastResult; // the agent's actual final output, pre-note
    S().setRun(id, { status, nextRunAt: undefined, ...(note ? { lastResult: note } : {}) });
    if (note) append(`— ${note} —`);
    // isolated runs leave their edits in the worktree for the user to review + merge
    const wt = S().runs[id]?.worktreePath;
    if (wt && (status === "done" || status === "crashloop" || status === "stopped"))
      append(`changes are isolated in ${wt} — review and merge when ready`);
    void recordRunEnd(id, status, note, true, prevResult); // history + outcome notification
  };

  const tick = async () => {
    if (ctrl.stop) return;
    if (ctrl.paused) {
      ctrl.timer = setTimeout(tick, 1000);
      return;
    }
    const def = S().loops[id];
    if (!def) return finish("stopped");
    let run = S().runs[id];

    // a scheduled loop can wait hours before its first fire — measure the time budget (and the
    // history duration) from first WORK, not from arming
    if (run.iteration === 0 && def.mode === "cron") {
      S().setRun(id, { startedAt: Date.now() });
      run = S().runs[id];
    }

    // guard: mandatory max iterations. If the tail of the run was failures, say so — "done"
    // would mask an error state.
    if (run.iteration >= def.stop.maxIterations)
      return finish(
        fails > 0 ? "error" : "done",
        fails > 0
          ? `reached ${def.stop.maxIterations} iterations (last ${fails} failed)`
          : `reached ${def.stop.maxIterations} iterations`,
      );
    // guard: time budget
    if (def.stop.timeBudgetMin && run.startedAt && Date.now() - run.startedAt > def.stop.timeBudgetMin * 60000)
      return finish("stopped", "time budget reached");
    // guard: token budget (claude + codex report usage per turn; gemini/grok/opencode don't, so it's a no-op there)
    if (def.stop.tokenBudget && (run.tokensUsed ?? 0) >= def.stop.tokenBudget)
      return finish("done", `token budget reached (${(run.tokensUsed ?? 0).toLocaleString()} tokens)`);

    const iter = run.iteration + 1;
    S().setRun(id, { status: "running", iteration: iter, lastRunAt: Date.now() });
    append(`\n— iteration ${iter} —`);
    S().pushEvent(id, newLoopEvent(iter, { kind: "iteration" }));
    const cont = def.session === "continue" && iter > 1; // first run seeds the session, rest continue it
    const { out, tokens, cost, failed } = await runIteration(def, runFolder, cont, streamAppend);
    if (ctrl.stop) return;
    // tally what this turn burned so the budget guard + UI have fresh numbers
    if (tokens || cost) {
      const cur = S().runs[id];
      S().setRun(id, { tokensUsed: (cur.tokensUsed ?? 0) + tokens, costUsed: (cur.costUsed ?? 0) + cost });
    }

    // a failed iteration retries with backoff instead of burning quota; N in a row stops the run
    if (failed) {
      fails += 1;
      if (fails >= FAIL_LIMIT) return finish("error", `${FAIL_LIMIT} consecutive failures`);
      const delay = RETRY_BASE_MS * 2 ** (fails - 1);
      append(`iteration failed — retrying in ${Math.round(delay / 1000)}s (${fails}/${FAIL_LIMIT})`);
      S().setRun(id, { lastResult: firstLine(out) || "iteration failed" });
      ctrl.timer = setTimeout(tick, delay);
      return;
    }
    fails = 0;

    // guard: no-progress / crash-loop
    const h = hash(out);
    const prevStale = S().runs[id].stale;
    const stale = h === lastHash || out.trim() === "" ? prevStale + 1 : 0;
    lastHash = h;
    S().setRun(id, { stale, lastResult: firstLine(out) || "(no output)" });
    if (def.stop.noProgress && stale >= NOPROGRESS_LIMIT)
      return finish("crashloop", `no progress for ${NOPROGRESS_LIMIT} iterations`);

    // stop: sentinel token in output
    if (def.stop.sentinel && out.includes(def.stop.sentinel)) return finish("done", "sentinel reached");

    // stop: the "until check" command now exits 0 (e.g. the build/tests finally pass)
    if (def.stop.untilCheck && def.stop.untilCheck.trim()) {
      const code = await runCheck(runFolder, def.stop.untilCheck).catch(() => -1);
      if (ctrl.stop) return;
      if (code === 0) return finish("done", "check passed");
      append(`check exited ${code}`);
    }

    // schedule the next iteration per mode
    if (def.mode === "manual") return finish("done", "ran once");
    if (def.mode === "interval") {
      ctrl.timer = setTimeout(tick, Math.max(1, def.intervalSec ?? 60) * 1000);
    } else if (def.mode === "until-done") {
      ctrl.timer = setTimeout(tick, UNTIL_DELAY_MS);
    } else if (def.mode === "cron") {
      const next = nextFire(def.schedule);
      S().setRun(id, { nextRunAt: next });
      armAt(ctrl, next, () => void tick());
    }
  };

  // "Interactive terminal" run mode: launch a REAL claude session in a PTY (engine-owned, see
  // loopTerm.ts) and auto-submit the task with /goal so it runs until the goal is met (or N turns).
  // It's interactive — the agent can ask / request approval and the user answers right in the
  // terminal — and a Notification hook pings HyprSpace whenever it needs the user.
  const runInteractive = async () => {
    if (ctrl.stop) return;
    const def = S().loops[id];
    if (!def) return finish("stopped");
    const rid = loopRunId(id);
    const env = useProjectConfigs.getState().getConfig(def.folder).env;

    // Notification hook (claude only) → marker file the engine polls to ping the user when it
    // needs them. codex/gemini have no hook system, so those run without the ping.
    let notifySettings = "";
    let notifyMarker = "";
    let notifyDone = "";
    if (isClaudeStream(def.provider)) {
      try {
        const f = await prepareNotifySettings(rid);
        notifySettings = f.settings;
        notifyMarker = f.marker;
        notifyDone = f.done;
      } catch {
        /* run without the notify hook if it couldn't be set up */
      }
    }
    if (ctrl.stop) return;

    // build the launch command typed into the shell. single-quote everything that carries user
    // text / paths so the target shell treats it literally (PowerShell expands $ and ` inside
    // double quotes, so single quotes are the safe wrap on both Windows and *nix).
    const q = (s: string) =>
      isWindows ? `'${s.replace(/'/g, "''")}'` : `'${s.replace(/'/g, "'\\''")}'`;
    const pmKey = def.permissionMode || "acceptEdits";
    const goal = def.prompt.replace(/\s+/g, " ").trim();
    let parts: string[];
    if (isClaudeStream(def.provider)) {
      // hand claude the goal as its opening prompt and let it run its normal agentic loop, exactly
      // like a human opening a session (the Orca approach). there is no `/goal` command in claude —
      // the old wrapper made claude choke on a fake slash command instead of doing the work. the
      // wall-clock time budget below is the backstop.
      const pm = claudePerm[pmKey] ?? "acceptEdits";
      parts = ["claude"];
      if (pm !== "default") parts.push("--permission-mode", pm);
      if (def.model) parts.push("--model", q(def.model));
      if (notifySettings) parts.push("--settings", q(notifySettings));
      parts.push(q(goal)); // the goal is claude's opening prompt, passed last like a manual launch
    } else if (def.provider === "codex") {
      // codex TUI takes the task as an initial prompt and keeps the session interactive
      parts = ["codex"];
      if (def.model) parts.push("-m", q(def.model));
      if (pmKey === "bypass") parts.push("--dangerously-bypass-approvals-and-sandbox");
      else parts.push("-s", codexSandbox[pmKey] ?? "workspace-write");
      parts.push(q(goal));
    } else {
      // gemini TUI: -i seeds the session with the task and stays interactive
      parts = ["gemini", "-i", q(goal)];
      if (def.model) parts.push("-m", q(def.model));
      parts.push("--approval-mode", geminiApproval[pmKey] ?? "default");
    }
    // "; exit" closes the shell when the CLI exits, so the PTY exit event fires and the run
    // actually completes — otherwise the loop shows "running" forever at a dead shell prompt
    const launchCmd = parts.join(" ") + "; exit";

    let ended = false;
    const done = (note: string) => {
      if (ended) return;
      ended = true;
      finish("done", note);
    };

    S().setRun(id, { status: "running", iteration: 0, startedAt: Date.now(), lastRunAt: Date.now() });
    append("interactive session — answer it in the terminal when it asks for input");
    try {
      await startLoopTerm(rid, runFolder, env, launchCmd, () => done("the session ended"));
    } catch (e) {
      return finish("error", `couldn't start Claude (${e})`);
    }
    if (ctrl.stop) return;

    // poll: the Stop marker → claude's turn ended, finish the run; the notify marker → "needs you" pings
    let seenNotif = 0;
    ctrl.pollTimer = setInterval(() => {
      if (ended) return;
      if (notifyDone) {
        void readFile(notifyDone)
          .then((s) => {
            if (s && s.trim() && !ended) done("goal finished");
          })
          .catch(() => {});
      }
      if (!notifyMarker) return;
      void readFile(notifyMarker)
        .then((s) => {
          const lines = (s || "").split("\n").filter((l) => l.trim());
          for (let i = seenNotif; i < lines.length; i++) {
            const msg = lines[i].slice(0, 200);
            useNotifications.getState().add({ title: `${def.name || "Loop"} needs you`, body: msg, kind: "info" });
            append(`needs you: ${msg}`);
            S().setRun(id, { lastResult: msg });
          }
          seenNotif = Math.max(seenNotif, lines.length);
        })
        .catch(() => {});
    }, 1500);

    // engine-side wall-clock cap (the /goal "stop after N turns" owns the turn cap)
    if (def.stop.timeBudgetMin && def.stop.timeBudgetMin > 0) {
      ctrl.timer = setTimeout(() => {
        if (!ended) finish("stopped", "time budget reached");
      }, def.stop.timeBudgetMin * 60000);
    }
  };

  const kickoff = () => {
    if (ctrl.stop) return;
    // cron waits for the first scheduled fire; the others start immediately
    if (def0.mode === "cron") {
      const next = nextFire(def0.schedule);
      S().setRun(id, { status: "running", nextRunAt: next });
      armAt(ctrl, next, () => void tick());
    } else {
      void tick();
    }
  };

  void (async () => {
    // compliance guard: the "claude" (API-key) backend needs a separate Anthropic API key. Without
    // one set, `claude -p` would silently fall back to the subscription — which is what "claude-sub"
    // is for explicitly, so refuse here and point the user at it (or at adding a key). Interactive
    // (pane) loops run the logged-in CLI directly on the subscription, so they never need a key.
    if (def0.provider === "claude" && def0.run !== "pane" && !(await secretHas("anthropic").catch(() => false))) {
      return finish("error", "add an Anthropic API key, or switch this loop to Claude (subscription)");
    }
    if (ctrl.stop) return;

    // worktree isolation: run the loop's edits in a throwaway branch off the folder's repo, so an
    // autonomous agent never touches the working tree until the user reviews the diff. falls back
    // to running in-place if the folder isn't a git repo.
    if (def0.worktree && def0.folder) {
      try {
        if (await gitIsRepo(def0.folder)) {
          const wt = await worktreeCreate(def0.folder, `loop-${id.slice(0, 8)}`);
          // record the worktree even if a stop raced the create, so it's surfaced, not orphaned
          runFolder = wt;
          S().setRun(id, { worktreePath: wt });
          if (ctrl.stop) return;
          append(`working in isolated worktree: ${wt}`);
        } else {
          append("not a git repo — running in place (no worktree isolation)");
        }
      } catch (e) {
        append(`worktree setup failed (${e}) — running in place`);
      }
    }
    if (ctrl.stop) return;

    // baseline of pre-existing uncommitted changes, so the end-of-run diff is a true delta
    try {
      const pre = await gitChanges(runFolder);
      baselines.set(id, new Map(pre.map((c) => [c.path, { a: c.added, r: c.removed }])));
    } catch {
      baselines.set(id, new Map());
    }
    if (ctrl.stop) return;

    // interactive-terminal automations run a real session in a PTY; everything else is headless.
    // a SCHEDULED interactive automation arms for its next fire instead of launching immediately.
    if (def0.run === "pane" && paneCapable(def0.provider)) {
      if (def0.mode === "cron") {
        const next = nextFire(def0.schedule);
        S().setRun(id, { status: "running", nextRunAt: next });
        armAt(ctrl, next, () => {
          S().setRun(id, { nextRunAt: undefined });
          void runInteractive();
        });
      } else {
        void runInteractive();
      }
    } else kickoff();
  })();
}

// open the isolated worktree folder for review (used by the UI's "review changes" button)
export function revealLoopWorktree(id: string) {
  const wt = useLoops.getState().runs[id]?.worktreePath;
  if (wt) void revealPath(wt).catch(() => {});
}

export function stopLoop(id: string) {
  const ctrl = ctrls.get(id);
  const wasActive = !!ctrl;
  if (ctrl) {
    ctrl.stop = true;
    clearTimers(ctrl);
    ctrls.delete(id);
  }
  void agentStop(loopRunId(id)).catch(() => {});
  stopLoopTerm(loopRunId(id));
  void cleanupHookRun(loopRunId(id)).catch(() => {});
  const prevResult = useLoops.getState().runs[id]?.lastResult;
  useLoops.getState().setRun(id, { status: "stopped", nextRunAt: undefined });
  // the user hit stop themselves — record it in history, but no notification needed.
  // (recordRunEnd itself skips runs that never fired, so stopping an armed cron is silent.)
  if (wasActive) void recordRunEnd(id, "stopped", "stopped manually", false, prevResult);
}

export function pauseLoop(id: string, paused: boolean) {
  const ctrl = ctrls.get(id);
  if (!ctrl) return;
  ctrl.paused = paused;
  useLoops.getState().setRun(id, { status: paused ? "paused" : "running" });
}
