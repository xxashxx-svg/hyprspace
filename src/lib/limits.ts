// Plan limits, shared by the titlebar meter and Settings → Usage so the two can never disagree.
//
// Three sources, best first: the account's live limits (polled by the meter, see UsageMeter.tsx and
// the cadence rule in devtools/live_usage.rs), what Claude's status line pushes each turn, and the
// windows Codex writes into its session files. This module only reads what those left in the
// stores; it never fetches anything itself.
import { useMemo } from "react";
import {
  useUsage,
  useCodexUsage,
  useLiveUsage,
  summarize,
  type UsageWindow,
  type ProviderBlock,
} from "../stores/usage";
import { relTime } from "./time";

// Once resets_at passes, the window has rolled over and whatever we last heard is the OLD window's
// final number, usually near 100%. Claude only tells us the new figure on the next turn, so until
// then we know nothing and must say so rather than showing a stale 100% in red.
export const expired = (w?: UsageWindow) => !!w && (!!w.stale || (!!w.resetsAt && w.resetsAt <= Date.now()));

// claude's own warning shape: it compares how much you've spent against how far through the window
// you are, not against a flat line. 89% with hours left is a problem; 89% with minutes left isn't.
// Pace needs the window's real length (judging a 7-day window against 5 hours makes everything
// look critical), so without a known length we fall back to flat thresholds.
export function tone(pct: number, w?: UsageWindow): "" | "warn" | "crit" {
  if (expired(w)) return "";
  // the provider's own severity beats anything we can infer, so take it when it's there
  if (w?.severity === "critical") return "crit";
  if (w?.severity === "warning") return "warn";
  if (w?.severity === "normal" && pct < 90) return "";
  if (pct >= 90) return "crit";
  const left = timeLeftPct(w);
  if (left !== undefined) {
    const elapsed = 100 - left;
    if (pct - elapsed > 14) return "crit";
    if (pct - elapsed > 4) return "warn";
  }
  return pct >= 75 ? "warn" : "";
}

/** How much of the window's time is still to come, 0 to 100. Undefined without a known length. */
export function timeLeftPct(w?: UsageWindow): number | undefined {
  if (!w?.windowMs || !w.resetsAt) return undefined;
  const left = w.resetsAt - Date.now();
  if (left <= 0 || left > w.windowMs) return undefined;
  return (left / w.windowMs) * 100;
}

/** "4h 42m", "4d 1h", or "resetting" once it's due */
export function resetLabel(w?: UsageWindow): string {
  if (!w?.resetsAt) return "";
  const ms = w.resetsAt - Date.now();
  if (ms <= 0) return "resetting";
  const m = Math.round(ms / 60000);
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  return h < 48 ? `${h}h ${m % 60}m` : `${Math.floor(h / 24)}d ${h % 24}h`;
}

/** The best limits we have for Claude and Codex right now, plus what to say about their age. */
export function useLimits() {
  const byPane = useUsage((s) => s.byPane);
  const liveClaude = useLiveUsage((s) => s.claude);
  const liveClaudeNote = useLiveUsage((s) => s.claudeNote);
  const liveCodex = useLiveUsage((s) => s.codex);
  const liveCodexNote = useLiveUsage((s) => s.codexNote);
  const codexFromFiles = useCodexUsage((s) => s.codex);

  const sum = useMemo(() => summarize(byPane), [byPane]);

  const fromStatusLine: ProviderBlock | null = useMemo(() => {
    if (!sum) return null;
    const windows = [
      ...(sum.five ? [{ key: "five_hour", label: "Session · 5h", win: sum.five }] : []),
      ...sum.others,
    ];
    // The header chip: the model in use. Names carry marketing suffixes ("Opus 5 (1M context)") and
    // panes can be on different models, so trim the parenthetical and count the rest rather than
    // joining them — a long string here stretched the whole popover.
    const seen = [...new Set(sum.models.map((m) => m.replace(/\s*\(.*\)\s*$/, "").trim()))];
    const plan = seen.length ? seen[0] + (seen.length > 1 ? ` +${seen.length - 1}` : "") : undefined;
    return windows.length ? { id: "claude", label: "Claude", plan, windows } : null;
  }, [sum]);

  // The live reading wins, but the status line still knows which model is running right now, which
  // the endpoint never says — so keep that label when we have it.
  const claude: ProviderBlock | null = useMemo(() => {
    if (!liveClaude) return fromStatusLine;
    return { ...liveClaude, plan: fromStatusLine?.plan ?? liveClaude.plan, note: liveClaudeNote };
  }, [liveClaude, liveClaudeNote, fromStatusLine]);

  // the live reading when the endpoint answered, else whatever the rollout files had
  const codex = liveCodex ?? codexFromFiles;
  // codex only records its windows mid-session, so a number from its files can be weeks old
  const codexAge = !liveCodex && codexFromFiles?.updatedAt ? relTime(codexFromFiles.updatedAt) : undefined;
  const codexNote = liveCodex?.note ?? (codexAge ? (codexAge === "now" ? "Just updated." : `As of ${codexAge} ago.`) : undefined);

  return {
    claude,
    codex,
    codexNote,
    /** no Claude pane has reported in a while, so the status-line numbers may be old */
    claudeStale: !!sum?.stale && !liveClaude,
    /** why a provider has no numbers at all, when its endpoint said (signed out, rate limited) */
    claudeMissing: !claude ? liveClaudeNote : undefined,
    codexMissing: !codex ? liveCodexNote : undefined,
  };
}
