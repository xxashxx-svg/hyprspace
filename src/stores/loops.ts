// Automations — agents that run on a schedule, on an interval, or when you hit Run. An automation
// is a saved definition (persisted as "loops"); its live runtime state (running / last output) is
// in-memory only. The engine that drives them is lib/automations.ts.
import { create } from "zustand";
import { saveState, loadState } from "../api";

export type LoopMode = "cron" | "interval" | "until-done" | "manual"; // until-done is legacy — treated as manual

// when a run is allowed to give up — the engine enforces a wall-clock budget (defaulted if unset)
export interface LoopStop {
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
  enabled: boolean; // scheduled automations with this set arm themselves when the app opens
  folder: string; // cwd the run happens in
  // pane-based claude on your logged-in CLI is the only backend — old headless/codex defs are
  // migrated onto it on load
  provider: "claude";
  prompt: string; // the task, typed into the agent's TUI once it's up
  mode: LoopMode;
  schedule?: ScheduleCfg; // for mode "cron"
  intervalSec?: number; // for mode "interval"
  worktree: boolean; // run edits in a throwaway git worktree + surface a reviewable diff
  permissionMode?: string; // claude --permission-mode (acceptEdits / plan / bypass / default)
  stop: LoopStop;
}

export type LoopStatus = "idle" | "running" | "paused" | "stopped" | "done" | "error" | "crashloop";

// in-memory runtime for an automation while the app is open
export interface LoopRun {
  status: LoopStatus;
  startedAt?: number;
  lastRunAt?: number;
  nextRunAt?: number; // for scheduled/interval automations
  lastResult?: string; // short summary of what the agent last did
  worktreePath?: string; // isolated git worktree the run is editing in (when worktree mode is on)
  // the pane the run is happening in, so the UI can offer to jump straight to it
  wsId?: string;
  paneId?: string;
  logs: string[]; // engine log lines (capped)
}

// one finished run, kept across restarts (persisted as "loop-history", capped per automation)
export interface LoopHistoryEntry {
  id: string;
  startedAt: number;
  endedAt: number;
  status: LoopStatus;
  iterations: number;
  note?: string; // why it stopped ("hit the 60-minute budget", …)
  lastResult?: string;
  tokensUsed?: number;
  costUsed?: number;
  worktreePath?: string;
  filesChanged?: number;
  additions?: number;
  deletions?: number;
}

const LOG_CAP = 4000;
const HISTORY_CAP = 50; // per automation

function blankRun(): LoopRun {
  return { status: "idle", logs: [] };
}

// a sane default automation, so "new automation" starts valid
export function newLoop(folder: string): LoopDef {
  return {
    id: crypto.randomUUID(),
    name: "",
    enabled: false,
    folder,
    provider: "claude",
    prompt: "",
    mode: "manual",
    worktree: true,
    permissionMode: "acceptEdits",
    intervalSec: 60,
    schedule: { everyMin: 60 },
    stop: { timeBudgetMin: 60 },
  };
}

interface LoopState {
  loops: Record<string, LoopDef>;
  runs: Record<string, LoopRun>;
  history: Record<string, LoopHistoryEntry[]>; // loopId → newest-first finished runs
  loaded: boolean;
  load: () => Promise<void>;
  upsert: (def: LoopDef) => void;
  remove: (id: string) => void;
  // runtime mutators (used by the engine)
  setRun: (id: string, patch: Partial<LoopRun>) => void;
  appendLog: (id: string, line: string) => void;
  appendLogs: (id: string, lines: string[]) => void;
  resetRun: (id: string) => void;
  addHistory: (loopId: string, entry: LoopHistoryEntry) => void;
}

// upsert fires per keystroke of the automation editor, and each persist is a full-store crash-safe
// disk write — debounce it (trailing), same as settingsSync. structural ops flush right away, and
// the hide/close listeners below make sure a pending write never gets lost.
let persistTimer: ReturnType<typeof setTimeout> | undefined;
function persistNow() {
  clearTimeout(persistTimer);
  persistTimer = undefined;
  void saveState("loops", JSON.stringify(useLoops.getState().loops)).catch(() => {});
}
function persistSoon() {
  clearTimeout(persistTimer);
  persistTimer = setTimeout(persistNow, 300);
}

export const useLoops = create<LoopState>()((set, get) => {
  const persistHistory = () =>
    void saveState("loop-history", JSON.stringify(get().history)).catch(() => {});
  return {
    loops: {},
    runs: {},
    history: {},
    loaded: false,

    load: async () => {
      if (get().loaded) return;
      const raw = await loadState("loops").catch(() => null);
      if (raw) {
        try {
          const c = JSON.parse(raw);
          if (c && typeof c === "object") {
            // migrate old defs onto the pane-based claude engine: retired providers fold to claude,
            // and the retired stop guards (maxIterations & co) reduce to the wall-clock budget
            for (const d of Object.values(c as Record<string, LoopDef>)) {
              if (!d) continue;
              d.provider = "claude";
              d.stop = { timeBudgetMin: d.stop && typeof d.stop === "object" ? d.stop.timeBudgetMin : undefined };
            }
            set({ loops: c as Record<string, LoopDef> });
          }
        } catch {
          /* ignore a bad blob */
        }
      }
      const hist = await loadState("loop-history").catch(() => null);
      if (hist) {
        try {
          const h = JSON.parse(hist);
          if (h && typeof h === "object") set({ history: h as Record<string, LoopHistoryEntry[]> });
        } catch {
          /* ignore a bad blob */
        }
      }
      set({ loaded: true });
    },

    upsert: (def) => {
      set((s) => ({ loops: { ...s.loops, [def.id]: def } }));
      persistSoon();
    },
    remove: (id) => {
      set((s) => {
        const loops = { ...s.loops };
        delete loops[id];
        const runs = { ...s.runs };
        delete runs[id];
        const history = { ...s.history };
        delete history[id];
        return { loops, runs, history };
      });
      persistNow(); // structural — don't sit on a delete
      persistHistory();
    },

    setRun: (id, patch) =>
      set((s) => ({ runs: { ...s.runs, [id]: { ...(s.runs[id] ?? blankRun()), ...patch } } })),
    appendLogs: (id, lines) =>
      set((s) => {
        if (lines.length === 0) return {};
        const cur = s.runs[id] ?? blankRun();
        const logs = [...cur.logs, ...lines];
        if (logs.length > LOG_CAP) logs.splice(0, logs.length - LOG_CAP);
        return { runs: { ...s.runs, [id]: { ...cur, logs } } };
      }),
    appendLog: (id, line) => get().appendLogs(id, [line]),
    resetRun: (id) => set((s) => ({ runs: { ...s.runs, [id]: blankRun() } })),
    addHistory: (loopId, entry) => {
      set((s) => {
        const list = [entry, ...(s.history[loopId] ?? [])].slice(0, HISTORY_CAP);
        return { history: { ...s.history, [loopId]: list } };
      });
      persistHistory();
    },
  };
});

// land a pending debounced write before the window goes away (same guards App.tsx uses)
const flushPersist = () => {
  if (persistTimer) persistNow();
};
window.addEventListener("blur", flushPersist);
window.addEventListener("pagehide", flushPersist);
document.addEventListener("visibilitychange", () => {
  if (document.visibilityState === "hidden") flushPersist();
});
