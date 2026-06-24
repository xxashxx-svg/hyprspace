// Loops — HyprSpace's headline feature: agents that run on a schedule, on an interval, or in a
// re-prompt-until-done loop. A loop is a saved definition (persisted as "loops"); its live runtime
// state (running / iteration / last output) is in-memory only.
import { create } from "zustand";
import { saveState, loadState } from "../api";

export type LoopMode = "cron" | "interval" | "until-done" | "manual";
export type SessionMode = "fresh" | "continue"; // fresh claude -p each iteration vs one long-lived session
export type RunMode = "headless" | "pane";

// when a loop is allowed to stop — at least one hard limit is always required (no infinite loops)
export interface LoopStop {
  maxIterations: number; // hard cap, always set (the mandatory stop)
  untilCheck?: string; // shell command run after each iteration; exit 0 = done, stop
  sentinel?: string; // if the agent's output contains this token (e.g. "LOOP_DONE"), stop
  noProgress: boolean; // auto-stop if N iterations in a row change nothing / keep erroring
  tokenBudget?: number; // best-effort cap on total output tokens across the loop
  timeBudgetMin?: number; // wall-clock cap in minutes
}

export interface ScheduleCfg {
  everyMin?: number; // simple interval, in minutes (cron mode)
  dailyAt?: string; // "HH:MM" local (cron mode)
  cron?: string; // raw 5-field cron expression (cron mode)
}

export interface LoopDef {
  id: string;
  name: string;
  enabled: boolean;
  folder: string; // cwd the loop runs in
  provider: "claude" | "gemini" | "codex";
  model?: string; // model override
  prompt: string; // the instruction sent each iteration
  mode: LoopMode;
  schedule?: ScheduleCfg; // for mode "cron"
  intervalSec?: number; // for mode "interval"
  session: SessionMode; // fresh per iteration, or continue one session
  run: RunMode; // headless (logs + pill) or its own pane
  worktree: boolean; // run edits in a throwaway git worktree + surface a reviewable diff
  permissionMode?: string; // claude --permission-mode (acceptEdits / plan / bypass / default)
  stop: LoopStop;
}

export type LoopStatus = "idle" | "running" | "paused" | "stopped" | "done" | "error" | "crashloop";

// in-memory runtime for a loop while the app is open
export interface LoopRun {
  status: LoopStatus;
  iteration: number;
  startedAt?: number;
  lastRunAt?: number;
  nextRunAt?: number; // for scheduled/interval loops
  lastResult?: string; // short summary of the last iteration
  tokensUsed?: number;
  stale: number; // consecutive no-change iterations (drives the crash-loop guard)
  logs: string[]; // captured output lines (capped)
}

const LOG_CAP = 4000;
export const loopRunId = (id: string) => `loop:${id}`;

function blankRun(): LoopRun {
  return { status: "idle", iteration: 0, stale: 0, logs: [] };
}

// a sane default loop, so "new loop" starts valid (a stop condition is mandatory)
export function newLoop(folder: string): LoopDef {
  return {
    id: crypto.randomUUID(),
    name: "",
    enabled: false,
    folder,
    provider: "claude",
    prompt: "",
    mode: "until-done",
    session: "fresh",
    run: "headless",
    worktree: true,
    permissionMode: "acceptEdits",
    intervalSec: 60,
    schedule: { everyMin: 60 },
    stop: { maxIterations: 10, noProgress: true },
  };
}

interface LoopState {
  loops: Record<string, LoopDef>;
  runs: Record<string, LoopRun>;
  loaded: boolean;
  load: () => Promise<void>;
  upsert: (def: LoopDef) => void;
  remove: (id: string) => void;
  // runtime mutators (used by the loop runner)
  setRun: (id: string, patch: Partial<LoopRun>) => void;
  appendLog: (id: string, line: string) => void;
  resetRun: (id: string) => void;
}

export const useLoops = create<LoopState>()((set, get) => {
  const persist = () => void saveState("loops", JSON.stringify(get().loops)).catch(() => {});
  return {
    loops: {},
    runs: {},
    loaded: false,

    load: async () => {
      if (get().loaded) return;
      const raw = await loadState("loops").catch(() => null);
      if (raw) {
        try {
          const c = JSON.parse(raw);
          if (c && typeof c === "object") set({ loops: c as Record<string, LoopDef> });
        } catch {
          /* ignore a bad blob */
        }
      }
      set({ loaded: true });
    },

    upsert: (def) => {
      set((s) => ({ loops: { ...s.loops, [def.id]: def } }));
      persist();
    },
    remove: (id) => {
      set((s) => {
        const loops = { ...s.loops };
        delete loops[id];
        const runs = { ...s.runs };
        delete runs[id];
        return { loops, runs };
      });
      persist();
    },

    setRun: (id, patch) =>
      set((s) => ({ runs: { ...s.runs, [id]: { ...(s.runs[id] ?? blankRun()), ...patch } } })),
    appendLog: (id, line) =>
      set((s) => {
        const cur = s.runs[id] ?? blankRun();
        const logs = [...cur.logs, line];
        if (logs.length > LOG_CAP) logs.splice(0, logs.length - LOG_CAP);
        return { runs: { ...s.runs, [id]: { ...cur, logs } } };
      }),
    resetRun: (id) => set((s) => ({ runs: { ...s.runs, [id]: blankRun() } })),
  };
});
