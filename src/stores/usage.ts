// Live Claude usage per pane, fed by claude's own status line (see src-tauri/src/agenthook.rs).
//
// Claude hands its statusLine command a JSON blob every turn carrying the account's rate-limit
// windows and this pane's context fill. That's the whole source — no token is read and no
// API is called, which is the only way we're allowed to do this.
import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";

export interface UsageWindow {
  pct: number;
  resetsAt?: number; // unix ms
  /** how long the window is. Pace ("are you spending faster than the clock?") is meaningless
   *  without it — a 7-day window judged against 5 hours reads as catastrophic at 20%. */
  windowMs?: number;
}

export interface PaneUsage {
  at: number; // when this pane last reported
  model?: string;
  ctxPct?: number;
  /** account-wide rate-limit windows, keyed by claude's own name for them. Which ones exist depends
   *  on the plan — a Max account reports an all-models weekly and a Fable one, not an Opus one. */
  windows: Record<string, UsageWindow>;
}

/** claude's window keys, in the order we show them, with the wording its own UI uses */
export const WINDOW_LABEL: Record<string, string> = {
  five_hour: "Session · 5h",
  seven_day: "This week",
  seven_day_opus: "Opus this week",
  seven_day_sonnet: "Sonnet this week",
  seven_day_overage_included: "Fable 5",
  overage: "Usage credits",
};
const ORDER = Object.keys(WINDOW_LABEL);

const HOUR = 3600_000;
/** window length per claude key, so pace is judged against the right clock */
const WINDOW_MS: Record<string, number> = {
  five_hour: 5 * HOUR,
  seven_day: 168 * HOUR,
  seven_day_opus: 168 * HOUR,
  seven_day_sonnet: 168 * HOUR,
  seven_day_overage_included: 168 * HOUR,
};

export const windowLabel = (k: string) =>
  WINDOW_LABEL[k] ?? k.replace(/_/g, " ").replace(/^\w/, (c) => c.toUpperCase());

// a pane that hasn't reported in this long is no longer burning anything
export const STALE_MS = 30 * 60 * 1000;

const num = (v: unknown): number | undefined =>
  typeof v === "number" && Number.isFinite(v) ? v : undefined;

const clamp = (n: number) => Math.max(0, Math.min(100, n));

// claude writes resets_at as either unix seconds or an ISO string depending on the window
function resetMs(v: unknown): number | undefined {
  if (typeof v === "number" && Number.isFinite(v)) return v > 1e11 ? v : v * 1000;
  if (typeof v === "string") {
    const t = Date.parse(v);
    if (!Number.isNaN(t)) return t;
  }
  return undefined;
}

function win(raw: unknown, key: string): UsageWindow | undefined {
  if (!raw || typeof raw !== "object") return undefined;
  const o = raw as Record<string, unknown>;
  const pct = num(o.used_percentage) ?? num(o.utilization);
  if (pct === undefined) return undefined;
  // utilization comes through as a 0-1 fraction, used_percentage as 0-100
  return {
    pct: clamp(pct <= 1 ? pct * 100 : pct),
    resetsAt: resetMs(o.resets_at ?? o.resetsAt),
    windowMs: WINDOW_MS[key],
  };
}

function parse(sl: Record<string, unknown>): PaneUsage {
  const rl = (sl.rate_limits ?? {}) as Record<string, unknown>;
  const ctx = (sl.context_window ?? {}) as Record<string, unknown>;
  const model = sl.model;

  return {
    at: Date.now(),
    model:
      typeof model === "string"
        ? model
        : ((model ?? {}) as Record<string, unknown>).display_name as string | undefined,
    ctxPct: num(ctx.used_percentage) !== undefined ? clamp(num(ctx.used_percentage)!) : undefined,
    // take whatever windows this plan actually reports rather than assuming a fixed set
    windows: Object.fromEntries(
      Object.entries(rl)
        .map(([k, v]) => [k, win(v, k)] as const)
        .filter((e): e is readonly [string, UsageWindow] => !!e[1]),
    ),
  };
}

interface UsageState {
  byPane: Record<string, PaneUsage>;
  apply: (paneId: string, sl: Record<string, unknown>) => void;
  forget: (paneId: string) => void;
}

export const useUsage = create<UsageState>()((set) => ({
  byPane: {},
  apply: (paneId, sl) =>
    set((s) => ({ byPane: { ...s.byPane, [paneId]: parse(sl) } })),
  forget: (paneId) =>
    set((s) => {
      if (!s.byPane[paneId]) return {};
      const byPane = { ...s.byPane };
      delete byPane[paneId];
      return { byPane };
    }),
}));

export interface UsageSummary {
  five?: UsageWindow;
  /** every other window this plan reports, already ordered and labelled */
  others: { key: string; label: string; win: UsageWindow }[];
  at: number; // freshest report that carried windows
  stale: boolean;
}

/**
 * Fold every pane's report into one account-level picture.
 *
 * The rate-limit windows are account-wide, so the freshest pane wins rather than summing. There's no
 * honest per-pane split: claude reports one number for the account and never says which pane spent
 * what, so we don't guess.
 *
 * Not a zustand selector: it builds fresh objects, so calling it inside one would re-render forever.
 */
export function summarize(
  byPane: Record<string, PaneUsage>,
  now = Date.now(),
): UsageSummary | null {
  const all = Object.entries(byPane);
  if (!all.length) return null;

  // The account windows come from the freshest report THAT HAS THEM — a pane whose payload carried
  // no rate_limits must not blank out numbers another pane already gave us.
  const withWindows = all.filter(([, u]) => Object.keys(u.windows).length > 0);
  if (!withWindows.length) return null;
  const newest = withWindows.reduce((a, b) => (b[1].at > a[1].at ? b : a));
  const windows = newest[1].windows;
  const five = windows.five_hour;
  const others = ORDER.filter((k) => k !== "five_hour" && windows[k])
    .concat(Object.keys(windows).filter((k) => k !== "five_hour" && !ORDER.includes(k)))
    .map((key) => ({ key, label: windowLabel(key), win: windows[key] }));

  return {
    five,
    others,
    at: newest[1].at,
    stale: now - newest[1].at >= STALE_MS,
  };
}

// one listener for the whole app; started from main.tsx
let started = false;
export function initUsage() {
  if (started) return;
  started = true;
  void listen<{ paneId: string; statusLine: Record<string, unknown> }>("agent-usage", (e) => {
    if (!e.payload?.paneId) return;
    useUsage.getState().apply(e.payload.paneId, e.payload.statusLine ?? {});
  });
}

// ---- Codex ----
// Claude pushes; codex doesn't. Its windows only exist in session rollout files, so we pull them on
// a lazy timer and label them by their own window length rather than pretending they're 5h/weekly.
export interface ProviderBlock {
  id: string;
  label: string;
  plan?: string;
  updatedAt?: number; // unix ms, when the source file was last written
  windows: { key: string; label: string; win: UsageWindow }[];
}

function windowName(minutes: number): string {
  if (!minutes) return "Limit";
  if (minutes % 1440 === 0) {
    const d = minutes / 1440;
    return d === 7 ? "This week" : `${d} days`;
  }
  const h = Math.round(minutes / 60);
  return h === 5 ? "Session · 5h" : `${h} hours`;
}

interface CodexState {
  codex: ProviderBlock | null;
  setCodex: (b: ProviderBlock | null) => void;
}

export const useCodexUsage = create<CodexState>()((set) => ({
  codex: null,
  setCodex: (codex) => set({ codex }),
}));

/** shape whatever provider_usage_one("codex") gave us into the same block the popover renders */
export function toCodexBlock(u: {
  label: string;
  plan: string | null;
  updatedAt: number;
  primary: { usedPercent: number; windowMinutes: number; resetsAt: number } | null;
  secondary: { usedPercent: number; windowMinutes: number; resetsAt: number } | null;
} | null): ProviderBlock | null {
  if (!u) return null;
  const windows = (["primary", "secondary"] as const)
    .map((k) => ({ k, w: u[k] }))
    .filter((x) => !!x.w)
    .map(({ k, w }) => ({
      key: k,
      label: windowName(w!.windowMinutes),
      win: {
        pct: clamp(w!.usedPercent),
        resetsAt: w!.resetsAt ? w!.resetsAt * 1000 : undefined,
        windowMs: w!.windowMinutes ? w!.windowMinutes * 60_000 : undefined,
      },
    }));
  if (!windows.length) return null;
  return {
    id: "codex",
    label: u.label || "Codex",
    plan: u.plan ?? undefined,
    updatedAt: u.updatedAt ? u.updatedAt * 1000 : undefined,
    windows,
  };
}
