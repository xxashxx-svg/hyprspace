// Live agent state per pane, fed by Claude Code's hooks (see src-tauri/src/agenthook.rs).
// The sidebar reads this to show what each agent is doing and which sub-agents it has spawned.
import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";

export type AgentState = "working" | "waiting" | "done" | "idle";

export interface SubAgent {
  id: string;
  label: string; // the delegated task's description, or its agent type
  state: "working" | "idle";
  startedAt: number;
}

export interface PaneAgent {
  state: AgentState;
  since: number; // when it entered this state — drives the relative time + stale decay
  /** one line of "what it's doing" — the current tool, why it's waiting, or what it last said */
  activity?: string;
  /** live sub-agents, newest last. Only ones still running are kept. */
  subs: SubAgent[];
}

// "Edit sync.rs" / "Bash npm run build" — the tool plus its most identifying argument
function toolLabel(tool: string, input: Record<string, unknown>): string {
  const base = (v: unknown) => String(v ?? "").split(/[\/]/).filter(Boolean).pop() ?? "";
  const arg =
    base(input.file_path ?? input.path ?? input.notebook_path) ||
    String(input.command ?? input.pattern ?? input.query ?? input.description ?? "").trim();
  return arg ? `${tool} ${trim(arg, 44)}` : tool;
}

function trim(s: string, n: number): string {
  const one = s.replace(/\s+/g, " ").trim();
  return one.length > n ? one.slice(0, n - 1) + "…" : one;
}

// a "working" row older than this is almost certainly a hook we never got (crash, kill -9) rather
// than a half-hour turn — show it as idle instead of spinning forever.
export const STALE_MS = 30 * 60 * 1000;

interface AgentStatusState {
  byPane: Record<string, PaneAgent>;
  apply: (paneId: string, payload: Record<string, unknown>) => void;
  forget: (paneId: string) => void;
}

const blank = (): PaneAgent => ({ state: "idle", since: Date.now(), subs: [] });

export const useAgentStatus = create<AgentStatusState>()((set) => ({
  byPane: {},

  apply: (paneId, payload) =>
    set((s) => {
      const event = String(payload.hook_event_name ?? payload.hookEventName ?? "");
      const cur = s.byPane[paneId] ?? blank();
      const now = Date.now();
      let next: PaneAgent = cur;

      switch (event) {
        case "UserPromptSubmit":
          next = { ...cur, state: "working", since: now, activity: "thinking…" };
          break;
        case "Notification":
          // claude fires this when it wants you — a permission prompt or an idle input wait
          next = {
            ...cur,
            state: "waiting",
            since: now,
            activity: trim(String(payload.message ?? "waiting for you"), 60),
          };
          break;
        case "Stop":
          next = {
            ...cur,
            state: "done",
            since: now,
            // what it actually concluded, rather than just "done"
            activity: trim(String(payload.last_assistant_message ?? ""), 70) || undefined,
          };
          break;
        // Immediate feedback: the row should appear the instant a delegation happens, rather than
        // waiting for the next inventory. Tool is "Agent" on current claude, "Task" on older builds.
        case "PreToolUse": {
          const tool = String(payload.tool_name ?? "");
          const input = (payload.tool_input ?? {}) as Record<string, unknown>;
          if (tool !== "Agent" && tool !== "Task") {
            // any other tool just refreshes the activity line ("Edit sync.rs", "Bash cargo check")
            if (tool) {
              next = {
                ...cur,
                state: cur.state === "waiting" ? "working" : cur.state,
                activity: toolLabel(tool, input),
              };
            }
            break;
          }
          const label = String(input.description ?? input.subagent_type ?? "").trim() || "subagent";
          next = {
            ...cur,
            state: cur.state === "done" ? "working" : cur.state,
            activity: toolLabel(tool === "Agent" ? "Delegating" : tool, input),
            subs: [...cur.subs, { id: `pending-${now}`, label, state: "working", startedAt: now }],
          };
          break;
        }
        default:
          break;
      }

      // `background_tasks` is claude's own live inventory and rides along on Stop / SubagentStop.
      // Trust it over start/stop inference: SubagentStop fires while a backgrounded child is STILL
      // running, so retiring rows on that event made them vanish a split second after appearing.
      // An empty array is meaningful — it means nothing is alive.
      const inv = payload.background_tasks;
      if (Array.isArray(inv)) {
        const subs: SubAgent[] = [];
        for (const raw of inv) {
          const t = (raw ?? {}) as Record<string, unknown>;
          if (String(t.status ?? "") !== "running") continue;
          const id = String(t.id ?? "");
          if (!id) continue;
          subs.push({
            id,
            label:
              String(t.description ?? t.agent_type ?? "").trim() || "subagent",
            state: "working",
            // keep the original start time so the row's age doesn't reset on every inventory
            startedAt: cur.subs.find((x) => x.id === id)?.startedAt ?? now,
          });
        }
        next = { ...next, subs };
      }

      if (next === cur) return {};
      return { byPane: { ...s.byPane, [paneId]: next } };
    }),

  forget: (paneId) =>
    set((s) => {
      if (!s.byPane[paneId]) return {};
      const byPane = { ...s.byPane };
      delete byPane[paneId];
      return { byPane };
    }),
}));

/** what a row should actually render — folds stale "working" rows down to idle */
export function displayState(a: PaneAgent | undefined, now: number): AgentState {
  if (!a) return "idle";
  if ((a.state === "working" || a.state === "waiting") && now - a.since > STALE_MS) return "idle";
  return a.state;
}

// one listener for the whole app; started from main.tsx
let started = false;
export function initAgentStatus() {
  if (started) return;
  started = true;
  void listen<{ paneId: string; payload: Record<string, unknown> }>("agent-hook", (e) => {
    if (!e.payload?.paneId) return;
    useAgentStatus.getState().apply(e.payload.paneId, e.payload.payload ?? {});
  });
}
