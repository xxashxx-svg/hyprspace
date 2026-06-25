// The loop engine: drives a LoopDef per its mode (interval / until-done / cron / manual), runs each
// iteration through the headless agent runner (agent.rs), and enforces the safety guards. App-open
// lifetime — controllers live in a module map and are torn down on stop.
import {
  useLoops,
  loopRunId,
  type LoopDef,
  type LoopStatus,
  type ScheduleCfg,
} from "../stores/loops";
import { useProjectConfigs } from "../stores/projectConfig";
import { agentStart, agentStop, runCheck, worktreeCreate, gitIsRepo, revealPath, secretHas } from "../api";

const EXIT = "\0__agent_exit__"; // matches the sentinel agent.rs emits when a turn ends
const NOPROGRESS_LIMIT = 3; // consecutive unchanged/empty iterations → crash-loop stop
const UNTIL_DELAY_MS = 1200; // breather between until-done iterations

type Ctrl = { stop: boolean; paused: boolean; timer?: ReturnType<typeof setTimeout> };
const ctrls = new Map<string, Ctrl>();

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
function buildArgs(def: LoopDef, cont: boolean): string[] {
  const pm = def.permissionMode || "acceptEdits";
  if (def.provider === "claude") {
    // runs on the API key injected via secretsFor() — never the subscription.
    // stream-json (requires --verbose) emits one event per step so the log shows live progress.
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
  // gemini — `-p` flips to headless; empty value so only the stdin prompt counts. best-effort.
  const a = ["gemini"];
  if (def.model) a.push("-m", def.model);
  a.push("-p", "", "--approval-mode", geminiApproval[pm] ?? "default", "--skip-trust");
  if (cont) a.push("--resume", "latest");
  return a;
}

// keychain secrets injected as env for this backend. Claude loops run on your Anthropic API key
// (never the subscription token); Codex/Gemini use their own CLI login, so no key is injected.
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

// turn one claude stream-json line into a readable log line; carries the final result text when
// the run ends. non-JSON lines (e.g. stderr warnings) pass straight through.
function parseClaudeLine(line: string): { log?: string; result?: string } {
  let ev: Record<string, unknown>;
  try {
    ev = JSON.parse(line);
  } catch {
    return { log: line };
  }
  if (ev.type === "assistant") {
    const content = (ev.message as { content?: unknown[] } | undefined)?.content ?? [];
    const out: string[] = [];
    for (const b of content as Array<Record<string, unknown>>) {
      if (b.type === "text" && typeof b.text === "string" && b.text.trim()) out.push(b.text.trim());
      else if (b.type === "tool_use") out.push(`→ ${toolLabel(b.name as string, b.input as Record<string, unknown>)}`);
    }
    return { log: out.join("\n") || undefined };
  }
  if (ev.type === "result") {
    const r = typeof ev.result === "string" ? ev.result : "";
    const cost = typeof ev.total_cost_usd === "number" ? ` · $${(ev.total_cost_usd as number).toFixed(3)}` : "";
    return { result: r, log: `✓ done${cost}` };
  }
  return {};
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
function runIteration(def: LoopDef, folder: string, cont: boolean, append: (line: string) => void): Promise<string> {
  return new Promise((resolve) => {
    const rid = loopRunId(def.id);
    const env = useProjectConfigs.getState().getConfig(def.folder).env;
    const lines: string[] = []; // readable text, used for progress/sentinel checks
    let result = ""; // claude's final result text (when stream-json reports it)
    let done = false;
    const finish = () => {
      if (done) return;
      done = true;
      resolve(result || lines.join("\n"));
    };
    void agentStart(rid, folder, buildArgs(def, cont), env, secretsFor(def), def.prompt, (line) => {
      if (line === EXIT) {
        finish();
        return;
      }
      // claude streams JSON events → parse to readable progress; other backends are plain text
      if (def.provider === "claude") {
        const ev = parseClaudeLine(line);
        if (ev.result !== undefined) result = ev.result;
        if (ev.log) {
          lines.push(ev.log);
          append(ev.log);
        }
      } else {
        lines.push(line);
        append(line);
      }
    }).catch((e) => {
      append(`⚠ failed to start: ${e}`);
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
    if (ctrl.timer) clearTimeout(ctrl.timer);
    ctrls.delete(id);
    void agentStop(loopRunId(id)).catch(() => {});
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

    const iter = run.iteration + 1;
    S().setRun(id, { status: "running", iteration: iter, lastRunAt: Date.now() });
    append(`\n— iteration ${iter} —`);
    const cont = def.session === "continue" && iter > 1; // first run seeds the session, rest continue it
    const out = await runIteration(def, runFolder, cont, append);
    if (ctrl.stop) return;

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
    // compliance guard: a Claude loop must run on a separate Anthropic API key, never the
    // logged-in subscription. with no key set, `claude -p` would silently use the subscription —
    // so refuse to start until one is stored.
    if (def0.provider === "claude" && !(await secretHas("anthropic").catch(() => false))) {
      return finish("error", "add an Anthropic API key in Loops settings first");
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
    kickoff();
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
    if (ctrl.timer) clearTimeout(ctrl.timer);
    ctrls.delete(id);
  }
  void agentStop(loopRunId(id)).catch(() => {});
  useLoops.getState().setRun(id, { status: "stopped", nextRunAt: undefined });
}

export function pauseLoop(id: string, paused: boolean) {
  const ctrl = ctrls.get(id);
  if (!ctrl) return;
  ctrl.paused = paused;
  useLoops.getState().setRun(id, { status: paused ? "paused" : "running" });
}
