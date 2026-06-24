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
import { agentStart, agentStop } from "../api";

const EXIT = "\0__agent_exit__"; // matches the sentinel agent.rs emits when a turn ends
const NOPROGRESS_LIMIT = 3; // consecutive unchanged/empty iterations → crash-loop stop
const UNTIL_DELAY_MS = 1200; // breather between until-done iterations

type Ctrl = { stop: boolean; paused: boolean; timer?: ReturnType<typeof setTimeout> };
const ctrls = new Map<string, Ctrl>();

export const isLoopActive = (id: string) => ctrls.has(id);

// the provider argv for one iteration. claude is the primary path; the prompt is piped over stdin.
function buildArgs(def: LoopDef): string[] {
  if (def.provider === "claude") {
    const a = ["claude", "-p"];
    if (def.model) a.push("--model", def.model);
    a.push("--permission-mode", def.permissionMode || "acceptEdits");
    return a;
  }
  if (def.provider === "gemini") return def.model ? ["gemini", "-m", def.model] : ["gemini"];
  return ["codex", "exec"]; // codex headless (refined later)
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

// run ONE iteration headless (fresh claude -p), streaming lines to `append`; resolves with the
// full output once the turn's exit sentinel arrives. (continue-session mode is added separately.)
function runIteration(def: LoopDef, append: (line: string) => void): Promise<string> {
  return new Promise((resolve) => {
    const rid = loopRunId(def.id);
    const env = useProjectConfigs.getState().getConfig(def.folder).env;
    const lines: string[] = [];
    let done = false;
    const finish = () => {
      if (done) return;
      done = true;
      resolve(lines.join("\n"));
    };
    void agentStart(rid, def.folder, buildArgs(def), env, def.prompt, (line) => {
      if (line === EXIT) {
        finish();
        return;
      }
      lines.push(line);
      append(line);
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

  const finish = (status: LoopStatus, note?: string) => {
    ctrl.stop = true;
    if (ctrl.timer) clearTimeout(ctrl.timer);
    ctrls.delete(id);
    void agentStop(loopRunId(id)).catch(() => {});
    S().setRun(id, { status, nextRunAt: undefined, ...(note ? { lastResult: note } : {}) });
    if (note) append(`— ${note} —`);
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
    const out = await runIteration(def, append);
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

  // cron waits for the first scheduled fire; the others start immediately
  if (def0.mode === "cron") {
    const next = nextFire(def0.schedule);
    S().setRun(id, { status: "running", nextRunAt: next });
    ctrl.timer = setTimeout(tick, Math.max(1000, next - Date.now()));
  } else {
    void tick();
  }
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
