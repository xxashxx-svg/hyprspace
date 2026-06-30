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
  tokenBudget?: number; // best-effort cap on total tokens (input+output+cache) across the loop; claude only
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
  // "claude" = headless `claude -p` on an Anthropic API key; "claude-sub" = the same headless
  // `claude -p` but on your logged-in subscription (no key — the spawn-the-CLI path the panes use).
  // gemini/codex are headless too. ("claude-hooks" is a legacy value, migrated to "claude-sub".)
  provider: "claude" | "claude-sub" | "claude-hooks" | "gemini" | "codex" | "opencode";
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

// one entry in the live agentic transcript. tool events flip running → ok/error and gain a duration
// once their result comes back; thinking/text/result carry the model's words.
export type LoopEventKind = "iteration" | "tool" | "thinking" | "text" | "result" | "system";
export interface LoopEvent {
  id: string;
  iteration: number;
  kind: LoopEventKind;
  ts: number;
  tool?: string; // raw tool name ("Bash", "Edit", "Read"…) — the UI maps it to a friendly label + icon
  arg?: string; // the command / file path / pattern
  status?: "running" | "ok" | "error";
  durationMs?: number;
  text?: string; // thinking, assistant text, or the final result
}

// in-memory runtime for a loop while the app is open
export interface LoopRun {
  status: LoopStatus;
  iteration: number;
  startedAt?: number;
  lastRunAt?: number;
  nextRunAt?: number; // for scheduled/interval loops
  lastResult?: string; // short summary of the last iteration
  tokensUsed?: number; // running total of tokens across the loop (claude only)
  costUsed?: number; // running total in USD (claude reports total_cost_usd per turn)
  stale: number; // consecutive no-change iterations (drives the crash-loop guard)
  worktreePath?: string; // isolated git worktree the run is editing in (when worktree mode is on)
  logs: string[]; // captured output lines (capped) — feeds the classic logs panel
  events: LoopEvent[]; // structured transcript (capped) — feeds the live agentic view
}

const LOG_CAP = 4000;
const EVENT_CAP = 1500;
export const loopRunId = (id: string) => `loop:${id}`;

function blankRun(): LoopRun {
  return { status: "idle", iteration: 0, stale: 0, logs: [], events: [] };
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
  pushEvent: (id: string, ev: LoopEvent) => void;
  patchEvent: (id: string, eventId: string, patch: Partial<LoopEvent>) => void;
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
          if (c && typeof c === "object") {
            // migrate the retired interactive-TUI backend onto the clean headless one
            for (const d of Object.values(c as Record<string, LoopDef & { goalMode?: boolean }>)) {
              if (d && (d.provider as string) === "claude-hooks") d.provider = "claude-sub";
              if (d) delete d.goalMode;
            }
            set({ loops: c as Record<string, LoopDef> });
          }
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
    pushEvent: (id, ev) =>
      set((s) => {
        const cur = s.runs[id] ?? blankRun();
        const events = [...cur.events, ev];
        if (events.length > EVENT_CAP) events.splice(0, events.length - EVENT_CAP);
        return { runs: { ...s.runs, [id]: { ...cur, events } } };
      }),
    patchEvent: (id, eventId, patch) =>
      set((s) => {
        const cur = s.runs[id];
        if (!cur) return {};
        const i = cur.events.findIndex((e) => e.id === eventId);
        if (i < 0) return {}; // already aged out of the cap — nothing to update
        const events = cur.events.slice();
        events[i] = { ...events[i], ...patch };
        return { runs: { ...s.runs, [id]: { ...cur, events } } };
      }),
    resetRun: (id) => set((s) => ({ runs: { ...s.runs, [id]: blankRun() } })),
  };
});
