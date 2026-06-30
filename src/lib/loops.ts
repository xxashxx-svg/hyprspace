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
  revealPath,
  secretHas,
  prepareNotifySettings,
  readFile,
} from "../api";
import { startLoopTerm, stopLoopTerm } from "../terminal/loopTerm";

const EXIT = "\0__agent_exit__"; // matches the sentinel agent.rs emits when a turn ends
const NOPROGRESS_LIMIT = 3; // consecutive unchanged/empty iterations → crash-loop stop
const UNTIL_DELAY_MS = 1200; // breather between until-done iterations

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
    // `resume --last -` continues the last session, the trailing `-` makes it read stdin.
    if (cont) return ["codex", "exec", "resume", "--last", "-"];
    const a = ["codex", "exec"];
    if (def.model) a.push("-m", def.model);
    if (pm === "bypass") a.push("--dangerously-bypass-approvals-and-sandbox");
    else a.push("-s", codexSandbox[pm] ?? "workspace-write");
    a.push("--skip-git-repo-check");
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
  setResult: (r: string, tokens: number, cost: number) => void,
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
    setResult(r, tokens, costNum);
    S().pushEvent(loopId, newLoopEvent(iter, { kind: "result", text: r, arg: costStr.trim() || undefined }));
    append(`✓ done${costStr}`);
  }
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

// next fire time for a cron-mode loop. everyMin + dailyAt now; raw cron expression is a later add.
function nextFire(s?: ScheduleCfg): number {
  const now = Date.now();
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
  tokens: number; // tokens this iteration burned (claude only; 0 otherwise)
  cost: number; // USD this iteration cost (claude only)
}

function runIteration(def: LoopDef, folder: string, cont: boolean, append: (line: string) => void): Promise<IterResult> {
  return new Promise((resolve) => {
    const rid = loopRunId(def.id);
    const loopId = def.id;
    const iter = useLoops.getState().runs[loopId]?.iteration ?? 0;
    const env = useProjectConfigs.getState().getConfig(def.folder).env;
    const lines: string[] = []; // readable text, used for progress/sentinel checks
    const toolMap = new Map<string, { eventId: string; start: number }>(); // tool_use id → its event + start
    let result = ""; // claude's final result text (when stream-json reports it)
    let tokens = 0;
    let cost = 0;
    let done = false;
    const record = (line: string) => {
      lines.push(line);
      append(line);
    };
    const finish = () => {
      if (done) return;
      done = true;
      resolve({ out: result || lines.join("\n"), tokens, cost });
    };
    void agentStart(rid, folder, buildArgs(def, cont), env, secretsFor(def), def.prompt, (line) => {
      if (line === EXIT) {
        finish();
        return;
      }
      // both claude backends stream JSON events → structured transcript; others are plain text
      if (isClaudeStream(def.provider)) {
        handleClaudeLine(loopId, iter, line, toolMap, (r, t, c) => {
          result = r;
          tokens = t;
          cost = c;
        }, record);
      } else {
        useLoops.getState().pushEvent(loopId, newLoopEvent(iter, { kind: "text", text: line }));
        record(line);
      }
    }).catch((e) => {
      record(`⚠ failed to start: ${e}`);
      finish();
    });
  });
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
  let lastHash = "";
  let runFolder = def0.folder; // where iterations actually run — a worktree if isolation is on

  const finish = (status: LoopStatus, note?: string) => {
    ctrl.stop = true;
    clearTimers(ctrl);
    ctrls.delete(id);
    void agentStop(loopRunId(id)).catch(() => {});
    stopLoopTerm(loopRunId(id)); // tears down an interactive-loop PTY; no-op for headless loops
    S().setRun(id, { status, nextRunAt: undefined, ...(note ? { lastResult: note } : {}) });
    if (note) append(`— ${note} —`);
    // isolated runs leave their edits in the worktree for the user to review + merge
    const wt = S().runs[id]?.worktreePath;
    if (wt && (status === "done" || status === "crashloop" || status === "stopped"))
      append(`changes are isolated in ${wt} — review and merge when ready`);
  };

  const tick = async () => {
    if (ctrl.stop) return;
    if (ctrl.paused) {
      ctrl.timer = setTimeout(tick, 1000);
      return;
    }
    const def = S().loops[id];
    if (!def) return finish("stopped");
    const run = S().runs[id];

    // guard: mandatory max iterations
    if (run.iteration >= def.stop.maxIterations)
      return finish("done", `reached ${def.stop.maxIterations} iterations`);
    // guard: time budget
    if (def.stop.timeBudgetMin && run.startedAt && Date.now() - run.startedAt > def.stop.timeBudgetMin * 60000)
      return finish("stopped", "time budget reached");
    // guard: token budget (claude reports usage per turn; other backends never accumulate, so it's a no-op there)
    if (def.stop.tokenBudget && (run.tokensUsed ?? 0) >= def.stop.tokenBudget)
      return finish("done", `token budget reached (${(run.tokensUsed ?? 0).toLocaleString()} tokens)`);

    const iter = run.iteration + 1;
    S().setRun(id, { status: "running", iteration: iter, lastRunAt: Date.now() });
    append(`\n— iteration ${iter} —`);
    S().pushEvent(id, newLoopEvent(iter, { kind: "iteration" }));
    const cont = def.session === "continue" && iter > 1; // first run seeds the session, rest continue it
    const { out, tokens, cost } = await runIteration(def, runFolder, cont, append);
    if (ctrl.stop) return;
    // tally what this turn burned so the budget guard + UI have fresh numbers
    if (tokens || cost) {
      const cur = S().runs[id];
      S().setRun(id, { tokensUsed: (cur.tokensUsed ?? 0) + tokens, costUsed: (cur.costUsed ?? 0) + cost });
    }

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
      ctrl.timer = setTimeout(tick, Math.max(1000, next - Date.now()));
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
    const max = Math.max(1, def.stop.maxIterations || 10);

    // Notification hook → marker file the engine polls to ping the user when claude needs them
    let notifySettings = "";
    let notifyMarker = "";
    try {
      const f = await prepareNotifySettings(rid);
      notifySettings = f.settings;
      notifyMarker = f.marker;
    } catch {
      /* run without the notify hook if it couldn't be set up */
    }
    if (ctrl.stop) return;

    // build the launch command typed into the shell. /goal makes claude self-loop until the
    // condition holds; "or stop after N turns" is the hard backstop. single-quote everything that
    // carries user text / paths so the target shell treats it literally (PowerShell expands $ and `
    // inside double quotes, so single quotes are the safe wrap on both Windows and *nix).
    const q = (s: string) =>
      isWindows ? `'${s.replace(/'/g, "''")}'` : `'${s.replace(/'/g, "'\\''")}'`;
    const pm = claudePerm[def.permissionMode || "acceptEdits"] ?? "acceptEdits";
    const goal = def.prompt.replace(/\s+/g, " ").trim();
    const parts = ["claude", q(`/goal ${goal} or stop after ${max} turns`)];
    if (pm !== "default") parts.push("--permission-mode", pm);
    if (def.model) parts.push("--model", q(def.model));
    if (notifySettings) parts.push("--settings", q(notifySettings));
    const launchCmd = parts.join(" ");

    let ended = false;
    const done = (note: string) => {
      if (ended) return;
      ended = true;
      finish("done", note);
    };

    S().setRun(id, { status: "running", iteration: 0, startedAt: Date.now(), lastRunAt: Date.now() });
    append("interactive Claude session — answer it in the terminal when it asks for input");
    try {
      await startLoopTerm(rid, runFolder, env, launchCmd, () => done("the session ended"));
    } catch (e) {
      return finish("error", `couldn't start Claude (${e})`);
    }
    if (ctrl.stop) return;

    // poll the notify marker → raise a HyprSpace notification each time claude needs the user
    let seenNotif = 0;
    ctrl.pollTimer = setInterval(() => {
      if (ended || !notifyMarker) return;
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
      ctrl.timer = setTimeout(tick, Math.max(1000, next - Date.now()));
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
          if (ctrl.stop) return;
          runFolder = wt;
          S().setRun(id, { worktreePath: wt });
          append(`working in isolated worktree: ${wt}`);
        } else {
          append("not a git repo — running in place (no worktree isolation)");
        }
      } catch (e) {
        append(`worktree setup failed (${e}) — running in place`);
      }
    }
    if (ctrl.stop) return;
    // interactive-terminal claude loops run a real session in a PTY; everything else is headless
    if (def0.run === "pane" && isClaudeStream(def0.provider)) void runInteractive();
    else kickoff();
  })();
}

// open the isolated worktree folder for review (used by the UI's "review changes" button)
export function revealLoopWorktree(id: string) {
  const wt = useLoops.getState().runs[id]?.worktreePath;
  if (wt) void revealPath(wt).catch(() => {});
}

export function stopLoop(id: string) {
  const ctrl = ctrls.get(id);
  if (ctrl) {
    ctrl.stop = true;
    clearTimers(ctrl);
    ctrls.delete(id);
  }
  void agentStop(loopRunId(id)).catch(() => {});
  stopLoopTerm(loopRunId(id));
  useLoops.getState().setRun(id, { status: "stopped", nextRunAt: undefined });
}

export function pauseLoop(id: string, paused: boolean) {
  const ctrl = ctrls.get(id);
  if (!ctrl) return;
  ctrl.paused = paused;
  useLoops.getState().setRun(id, { status: paused ? "paused" : "running" });
}
