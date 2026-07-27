// The automation engine. An automation is a scheduled agent run in a REAL pane: when it fires we
// (optionally) cut a worktree, launch claude in a normal workspace pane, and — once its TUI is up —
// type the task in as keystrokes. The prompt never rides a shell command line, so there is nothing
// to quote and no shell (PowerShell / cmd / git-bash) that could misparse it.
//
// Why a real pane rather than a headless `claude -p`: the pane already carries everything — it's
// tiled in the grid, appears in the sidebar's agent tree with live status, feeds the usage meter,
// resumes with `--resume`, and runs on your subscription instead of an API key. The old engine
// duplicated all of that against a second, invisible runtime.
//
// The single hard rule (CLAUDE.md): an automation can never run forever. There is always a
// wall-clock budget, defaulted when the definition doesn't set one — and hitting it CLOSES the
// run's pane, so the agent dies with the run instead of grinding on unwatched.
import { useLoops, type LoopStatus, type ScheduleCfg } from "../stores/loops";
import { useWorkspaces } from "../stores/workspace";
import { useAgentStatus, displayState } from "../stores/agentStatus";
import { useUsage } from "../stores/usage";
import { useNotifications } from "../stores/notifications";
import { claudeCmd } from "../actions";
import type { ClaudePermission } from "../stores/settings";
import { worktreeCreate, revealPath, writePty } from "../api";
import { nextCron, cronValid } from "./cron";

const DEFAULT_BUDGET_MIN = 60; // the backstop when a definition names no time budget
const MAX_TIMEOUT_MS = 2_147_000_000; // setTimeout overflows past this and fires immediately
const BOOT_TIMEOUT_MS = 3 * 60_000; // how long a cold `claude` gets to render before we give up

interface Ctrl {
  stop: boolean;
  paused: boolean;
  timer?: ReturnType<typeof setTimeout>;
  budget?: ReturnType<typeof setTimeout>;
  unwatch?: () => void;
  wsId?: string;
  paneId?: string;
}

const ctrls = new Map<string, Ctrl>();
export const isLoopActive = (id: string) => ctrls.has(id);

// far-off fires must be chained — a single setTimeout past ~24.8 days wraps and fires at once
function armAt(ctrl: Ctrl, at: number, fn: () => void) {
  const delay = Math.max(0, at - Date.now());
  if (delay > MAX_TIMEOUT_MS) {
    ctrl.timer = setTimeout(() => armAt(ctrl, at, fn), MAX_TIMEOUT_MS);
    return;
  }
  ctrl.timer = setTimeout(fn, delay);
}

function clearTimers(ctrl: Ctrl) {
  clearTimeout(ctrl.timer);
  clearTimeout(ctrl.budget);
  ctrl.timer = undefined;
  ctrl.budget = undefined;
}

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
  return now + 60 * 60000;
}

/** the workspace this automation's folder belongs to, creating one (in the background) if needed */
function resolveWorkspace(folder: string): string {
  const W = useWorkspaces.getState();
  const key = (p: string) => p.replace(/[\\/]+$/, "").toLowerCase();
  const hit = W.workspaces.find((w) => w.kind !== "open" && w.cwd && key(w.cwd) === key(folder));
  if (hit) return hit.id;
  const name = folder.split(/[\\/]/).filter(Boolean).pop() || "Automation";
  // activate:false — a scheduled fire must never switch the space you're looking at
  return W.addWorkspace(name, folder, { activate: false });
}

export function startLoop(id: string) {
  if (ctrls.has(id)) return;
  const def0 = useLoops.getState().loops[id];
  if (!def0) return;

  const ctrl: Ctrl = { stop: false, paused: false };
  ctrls.set(id, ctrl);
  const S = () => useLoops.getState();
  const append = (line: string) => S().appendLog(id, line);

  S().resetRun(id);
  S().setRun(id, { status: "running", startedAt: Date.now() });

  let finished = false;

  const finish = (status: LoopStatus, note?: string) => {
    if (finished) return;
    finished = true;
    clearTimers(ctrl);
    ctrl.unwatch?.();
    ctrl.unwatch = undefined;

    const def = S().loops[id];
    const recurring = !ctrl.stop && !!def && (def.mode === "cron" || def.mode === "interval");
    // never leave a live agent behind: the pane dies on an error (that's the hard stop working) and
    // before a recurring re-arm (or every run would stack another tab). A finished one-shot keeps
    // its pane so you can read what the agent actually did.
    const keepPane = status === "done" && !recurring;
    if (!keepPane && ctrl.wsId && ctrl.paneId) {
      useWorkspaces.getState().removeSession(ctrl.wsId, ctrl.paneId);
    }
    if (!keepPane) {
      ctrl.wsId = undefined;
      ctrl.paneId = undefined;
    }

    const run = S().runs[id];
    if (note) append(`— ${note} —`);
    if (run?.worktreePath) append(`changes are isolated in ${run.worktreePath} — review and merge when ready`);

    S().addHistory(id, {
      id: crypto.randomUUID(),
      startedAt: run?.startedAt ?? Date.now(),
      endedAt: Date.now(),
      status,
      iterations: 1,
      note,
      lastResult: run?.lastResult,
      worktreePath: run?.worktreePath,
    });
    useNotifications.getState().add({
      title: `${def?.name || "Automation"} ${status === "done" ? "finished" : status}`,
      body: note || run?.lastResult || "",
    });

    if (recurring) {
      // a scheduled automation re-arms for its next fire instead of dying after one run
      finished = false;
      const at = def.mode === "interval" ? Date.now() + (def.intervalSec || 60) * 1000 : nextFire(def.schedule);
      S().setRun(id, { status: "running", nextRunAt: at, startedAt: undefined, wsId: undefined, paneId: undefined });
      append(`armed — next run ${new Date(at).toLocaleString()}`);
      armAt(ctrl, at, () => void fire());
    } else {
      ctrl.stop = true;
      ctrls.delete(id);
      S().setRun(id, {
        status,
        nextRunAt: undefined,
        ...(keepPane ? {} : { wsId: undefined, paneId: undefined }),
        ...(note ? { lastResult: note } : {}),
      });
    }
  };

  /**
   * Watch the pane's hook status. An automation is done when its agent reports `Stop` — but only
   * after we've seen it working, otherwise the pane's initial empty state reads as "done" the
   * instant we subscribe and the run ends before it begins.
   */
  const watchPane = (paneId: string) => {
    let sawWorking = false;
    let last: unknown;
    let lastState = "";
    const check = () => {
      if (ctrl.stop) return;
      const agent = useAgentStatus.getState().byPane[paneId];
      if (agent === last) return; // the store fires for every pane's hooks — only react to ours
      last = agent;
      const state = displayState(agent, Date.now());
      if (state === "working") sawWorking = true;
      if (agent?.activity) S().setRun(id, { lastResult: agent.activity });
      if (state === "waiting" && lastState !== "waiting") append("the agent is waiting for you — open the pane to answer");
      lastState = state;
      if (state === "done" && sawWorking) finish("done", agent?.activity || "the agent finished");
    };
    ctrl.unwatch = useAgentStatus.subscribe(check);
    check();
  };

  /**
   * Type the task into the pane's claude TUI once it's up. The TUI's status line reporting for
   * this pane is the "rendered" signal (hooks only fire after a prompt is submitted, so they can't
   * be it). If it never reports, the CLI didn't come up — erroring out beats typing a task at
   * whatever IS in that pane.
   */
  const typeTask = (paneId: string, task: string) => {
    const enc = new TextEncoder();
    let typed = false;
    ctrl.timer = setTimeout(() => {
      if (typed || ctrl.stop) return;
      typed = true;
      finish("error", "the agent never came up — is the claude CLI installed and signed in?");
    }, BOOT_TIMEOUT_MS);
    const ready = () => {
      if (typed || ctrl.stop) return;
      if (!useUsage.getState().byPane[paneId]) return;
      typed = true;
      clearTimeout(ctrl.timer);
      ctrl.timer = undefined;
      ctrl.unwatch?.();
      ctrl.unwatch = undefined;
      void writePty(paneId, enc.encode(task));
      // Enter goes separately a beat later so the TUI settles the pasted text first
      setTimeout(() => {
        if (!ctrl.stop) void writePty(paneId, enc.encode("\r"));
      }, 300);
      watchPane(paneId);
    };
    ctrl.unwatch = useUsage.subscribe(ready);
    ready();
  };

  const fire = async () => {
    if (ctrl.stop) return;
    if (ctrl.paused) {
      ctrl.timer = setTimeout(fire, 1000);
      return;
    }
    const def = S().loops[id];
    if (!def) return finish("stopped");

    S().setRun(id, { startedAt: Date.now(), nextRunAt: undefined });

    // isolate the run so it can't touch the working tree; a folder that isn't a git repo (or any
    // other worktree failure) just runs in place rather than killing the run
    let runFolder = def.folder;
    if (def.worktree) {
      try {
        runFolder = await worktreeCreate(def.folder, `auto-${(def.name || "run").toLowerCase().replace(/[^a-z0-9]+/g, "-").slice(0, 24)}`);
        S().setRun(id, { worktreePath: runFolder });
        append(`isolated in ${runFolder}`);
      } catch (e) {
        runFolder = def.folder;
        append(`no worktree (${e}) — running in ${def.folder}`);
      }
    }
    if (ctrl.stop) return;

    const wsId = resolveWorkspace(def.folder);
    if (!wsId) return finish("error", "couldn't find a workspace for this folder");

    // Launch it out of the way. A new *pane* would re-tile the grid and shove your agents around
    // even without stealing focus, so when the space already has panes we stack the run as a
    // background TAB on the last one — the layout doesn't move and the run waits behind a tab.
    // The pane is ephemeral: PaneGrid mounts it even in a space you haven't opened (without
    // spawning the space's OTHER panes), and it's never persisted — a saved automation pane would
    // relaunch its agent on the next app start.
    const W = useWorkspaces.getState();
    const ws = W.workspaces.find((w) => w.id === wsId);
    const anchor = ws?.sessions[ws.sessions.length - 1];
    const cmd = claudeCmd((def.permissionMode as ClaudePermission) || "acceptEdits");
    const opts = { focus: false, ephemeral: true };
    const paneId = anchor
      ? W.addTab(wsId, anchor.id, cmd, runFolder, opts)
      : W.addSession(wsId, cmd, runFolder, opts);
    ctrl.wsId = wsId;
    ctrl.paneId = paneId;
    S().setRun(id, { wsId, paneId }); // lets the page offer a jump-to-pane button
    append("running in a background tab — use “Watch” to jump to it");

    // the hard stop. Without it a hung agent would sit "running" forever — and finish() closes the
    // pane, so hitting the budget kills the agent rather than just flipping the card to error.
    const budgetMin = def.stop.timeBudgetMin && def.stop.timeBudgetMin > 0 ? def.stop.timeBudgetMin : DEFAULT_BUDGET_MIN;
    ctrl.budget = setTimeout(() => finish("error", `hit the ${budgetMin}-minute budget`), budgetMin * 60000);

    typeTask(paneId, def.prompt.replace(/\s+/g, " ").trim());
  };

  // scheduled automations arm for their next fire; everything else runs now
  if (def0.mode === "cron" || def0.mode === "interval") {
    const at = def0.mode === "interval" ? Date.now() + (def0.intervalSec || 60) * 1000 : nextFire(def0.schedule);
    S().setRun(id, { status: "running", nextRunAt: at });
    append(`armed — next run ${new Date(at).toLocaleString()}`);
    armAt(ctrl, at, () => void fire());
  } else {
    void fire();
  }
}

export function stopLoop(id: string) {
  const ctrl = ctrls.get(id);
  if (!ctrl) return;
  ctrl.stop = true;
  clearTimers(ctrl);
  ctrl.unwatch?.();
  ctrls.delete(id);
  // close the pane too — leaving a live agent behind would be an automation that never stops
  if (ctrl.wsId && ctrl.paneId) useWorkspaces.getState().removeSession(ctrl.wsId, ctrl.paneId);
  useLoops.getState().setRun(id, { status: "stopped", nextRunAt: undefined });
}

export function pauseLoop(id: string, paused: boolean) {
  const ctrl = ctrls.get(id);
  if (!ctrl) return;
  ctrl.paused = paused;
  useLoops.getState().setRun(id, { status: paused ? "paused" : "running" });
}

export function revealLoopWorktree(id: string) {
  const wt = useLoops.getState().runs[id]?.worktreePath;
  if (wt) void revealPath(wt).catch(() => {});
}
