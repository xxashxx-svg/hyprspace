// Titlebar chip + popover for live provider usage. Claude pushes its numbers at us (see
// src/stores/usage.ts); codex has to be polled, and only while a codex pane is actually open.
import { useEffect, useMemo, useRef, useState } from "react";
import {
  useUsage,
  useCodexUsage,
  useLiveUsage,
  summarize,
  toCodexBlock,
  toLiveBlock,
  type UsageWindow,
  type ProviderBlock,
} from "../stores/usage";
import { useWorkspaces } from "../stores/workspace";
import { claudeLiveUsage, codexLiveUsage, providerUsageOne } from "../api";
import { relTime } from "../lib/time";
import { PROVIDER_COLOR } from "../lib/brand";
import claudeLogo from "../assets/brand/claude.svg";
import openaiLogo from "../assets/brand/openai.svg";

const LOGO: Record<string, string> = { claude: claudeLogo, codex: openaiLogo };

const CODEX_POLL_MS = 60_000;
// Claude's usage endpoint rate limits per token and shares that bucket with Claude Code itself,
// so this cadence is deliberate: polling faster throttles the CLI as well as us. Codex gets the
// same number because its windows are 5 hours and a week — a faster refresh buys no accuracy and
// only spends requests. 20 an hour each, and none at all while the window is hidden.
const LIVE_POLL_MS = 180_000;
// long enough to read, short enough not to feel like waiting. Matches the close keyframe.
const CLOSE_MS = 120;

// claude's own warning shape: it compares how much you've spent against how far through the window
// you are, not against a flat line. 89% with hours left is a problem; 89% with minutes left isn't.
// Pace needs the window's real length — judging a 7-day window against 5 hours makes everything
// look critical — so without a known length we fall back to flat thresholds.
function tone(pct: number, w?: UsageWindow): "" | "warn" | "crit" {
  if (expired(w)) return "";
  // the provider's own severity beats anything we can infer, so take it when it's there
  if (w?.severity === "critical") return "crit";
  if (w?.severity === "warning") return "warn";
  if (w?.severity === "normal" && pct < 90) return "";
  if (pct >= 90) return "crit";
  const windowMs = w?.windowMs;
  const left = w?.resetsAt ? w.resetsAt - Date.now() : undefined;
  if (windowMs && left !== undefined && left > 0 && left <= windowMs) {
    const elapsed = ((windowMs - left) / windowMs) * 100;
    if (pct - elapsed > 14) return "crit";
    if (pct - elapsed > 4) return "warn";
  }
  return pct >= 75 ? "warn" : "";
}

const RANK: Record<string, number> = { "": 0, warn: 1, crit: 2 };

// Once resets_at passes, the window has rolled over and whatever we last heard is the OLD window's
// final number — usually near 100%. Claude only tells us the new figure on the next turn, so until
// then we know nothing and must say so rather than showing a stale 100% in red.
const expired = (w?: UsageWindow) => !!w && (!!w.stale || (!!w.resetsAt && w.resetsAt <= Date.now()));

function resetLabel(w?: UsageWindow): string {
  if (!w?.resetsAt) return "";
  const ms = w.resetsAt - Date.now();
  if (ms <= 0) return "resetting";
  const m = Math.round(ms / 60000);
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  return h < 48 ? `${h}h ${m % 60}m` : `${Math.floor(h / 24)}d ${h % 24}h`;
}

function Ring({ pct, tone }: { pct: number; tone: string }) {
  const r = 6.6;
  const c = 2 * Math.PI * r;
  return (
    <svg className={`um-ring ${tone}`} width="16" height="16" viewBox="0 0 16 16">
      <circle className="um-ring-tr" cx="8" cy="8" r={r} fill="none" strokeWidth="2.4" />
      <circle
        className="um-ring-fg"
        cx="8"
        cy="8"
        r={r}
        fill="none"
        strokeWidth="2.4"
        strokeLinecap="round"
        strokeDasharray={`${((c * pct) / 100).toFixed(1)} ${c.toFixed(1)}`}
      />
    </svg>
  );
}

/** one provider: a header, then a row per window it reports */
function Section({ block, note }: { block: ProviderBlock; note?: string }) {
  return (
    <section className="um-card" style={{ "--brand": PROVIDER_COLOR[block.id] ?? "var(--text-1)" } as React.CSSProperties}>
      <header className="um-head">
        {LOGO[block.id] && (
          <span className="um-mark">
            <img src={LOGO[block.id]} alt="" />
          </span>
        )}
        <span className="um-name">{block.label}</span>
        {block.plan && (
          <span className="um-plan" title={block.plan}>
            {block.plan}
          </span>
        )}
      </header>
      <div className="um-body">
        {block.windows.map(({ key, label, win }) => {
          const gone = expired(win);
          const t = tone(win.pct, win);
          return (
            <div className="um-win" key={key}>
              <div className="um-win-top">
                <span className="um-lbl">{label}</span>
                <span className={`um-val ${t}${gone ? " muted" : ""}`}>{gone ? "—" : `${Math.round(win.pct)}%`}</span>
              </div>
              <div className={`um-bar ${t}`}>
                <i style={{ width: `${gone ? 0 : win.pct}%` }} />
              </div>
              <div className="um-foot">
                {gone ? "window reset, updates next turn" : win.resetsAt ? `resets in ${resetLabel(win)}` : ""}
              </div>
            </div>
          );
        })}
        {block.extra && (
          <div className="um-win">
            <div className="um-win-top">
              <span className="um-lbl">Extra usage</span>
              <span className="um-val">{Math.round(block.extra.percent)}%</span>
            </div>
            <div className="um-bar">
              <i style={{ width: `${block.extra.percent}%` }} />
            </div>
            <div className="um-foot">
              {block.extra.currency === "USD" ? "$" : ""}
              {block.extra.used.toFixed(2)} of {block.extra.currency === "USD" ? "$" : ""}
              {block.extra.limit.toFixed(2)} this month
            </div>
          </div>
        )}
        {note && <div className="um-note">{note}</div>}
      </div>
    </section>
  );
}

export function UsageMeter() {
  const byPane = useUsage((s) => s.byPane);
  const codexFromFiles = useCodexUsage((s) => s.codex);
  // a plain boolean, so this doesn't re-render on unrelated workspace churn
  const hasCodex = useWorkspaces((s) =>
    s.workspaces.some((w) => w.sessions.some((x) => x.provider === "codex")),
  );
  const [open, setOpen] = useState(false);
  // the panel has to stay mounted while it animates out, so closing is a state of its own
  const [closing, setClosing] = useState(false);
  const closeTimer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const [, bump] = useState(0);
  const ref = useRef<HTMLDivElement>(null);

  const close = () => {
    if (closeTimer.current) return; // already on the way out
    setClosing(true);
    closeTimer.current = setTimeout(() => {
      closeTimer.current = undefined;
      setClosing(false);
      setOpen(false);
    }, CLOSE_MS);
  };
  const toggle = () => {
    if (closeTimer.current) return;
    if (open) close();
    else setOpen(true);
  };
  useEffect(() => () => clearTimeout(closeTimer.current), []);

  // reset countdowns are the only thing that moves between reports
  useEffect(() => {
    const t = setInterval(() => bump((n) => n + 1), 30_000);
    return () => clearInterval(t);
  }, []);

  // nothing pushes codex usage, so poll it — but only while a codex pane exists, and lazily: these
  // windows are days long, so re-reading session files any faster would be pure waste.
  useEffect(() => {
    if (!hasCodex) {
      useCodexUsage.getState().setCodex(null);
      return;
    }
    let dead = false;
    const pull = () =>
      providerUsageOne("codex")
        .then((u) => {
          if (!dead) useCodexUsage.getState().setCodex(toCodexBlock(u));
        })
        .catch(() => {});
    void pull();
    const t = setInterval(() => void pull(), CODEX_POLL_MS);
    return () => {
      dead = true;
      clearInterval(t);
    };
  }, [hasCodex]);

  // The live endpoints know the account's limits whether or not an agent is running, so they are
  // the primary source. Both no-op cheaply when that CLI was never signed in on this machine.
  useEffect(() => {
    let dead = false;
    const pull = (
      id: "claude" | "codex",
      label: string,
      call: () => Promise<import("../api").LiveUsage>,
    ) =>
      call()
        .then((u) => {
          if (dead) return;
          useLiveUsage.getState().setLive(id, toLiveBlock(id, label, u), u.note ?? undefined);
        })
        .catch(() => {});

    const tick = (id: "claude" | "codex", label: string, call: () => Promise<import("../api").LiveUsage>) => {
      if (document.hidden) return;
      void pull(id, label, call);
    };
    tick("claude", "Claude", claudeLiveUsage);
    tick("codex", "Codex", codexLiveUsage);
    const a = setInterval(() => tick("claude", "Claude", claudeLiveUsage), LIVE_POLL_MS);
    // staggered so the two never fire in the same instant
    const b = setInterval(() => tick("codex", "Codex", codexLiveUsage), LIVE_POLL_MS + 7_000);
    return () => {
      dead = true;
      clearInterval(a);
      clearInterval(b);
    };
  }, []);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) close();
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && close();
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const sum = useMemo(() => summarize(byPane), [byPane]);
  const liveClaude = useLiveUsage((s) => s.claude);
  const liveCodex = useLiveUsage((s) => s.codex);
  const liveClaudeNote = useLiveUsage((s) => s.claudeNote);

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

  // The ring follows the most urgent window across every provider, otherwise it would sit calmly on
  // claude's session limit while codex was the thing about to run out. A window that is already
  // spent is skipped: it can't move and you can't act on it, so reporting it would hide the ones
  // you can still watch. Only when everything is spent does it report a spent one.
  const worst = useMemo(() => {
    // an expired window tells us nothing, so it can't be the thing the ring reports
    const all = [...(claude?.windows ?? []), ...(codex?.windows ?? [])].filter((w) => !expired(w.win));
    if (!all.length) return null;
    const pick = (list: typeof all) =>
      list
        .map((w) => ({ ...w, t: tone(w.win.pct, w.win) }))
        .reduce((a, b) =>
          RANK[b.t] > RANK[a.t] || (RANK[b.t] === RANK[a.t] && b.win.pct > a.win.pct) ? b : a,
        );
    const live = all.filter((w) => w.win.pct < 100);
    return pick(live.length ? live : all);
  }, [claude, codex]);

  const anyWindow = (claude?.windows.length ?? 0) + (codex?.windows.length ?? 0) > 0;
  if (!worst && !anyWindow) return null;

  // codex only records its windows mid-session, so a number can easily be weeks old — say so
  const codexAge = !liveCodex && codexFromFiles?.updatedAt ? relTime(codexFromFiles.updatedAt) : undefined;
  const codexNote =
    liveCodex?.note ?? (codexAge ? (codexAge === "now" ? "Just updated." : `As of ${codexAge} ago.`) : undefined);

  return (
    <div className="um" ref={ref}>
      <button
        className={`um-chip ${worst?.t ?? ""}${!worst || sum?.stale ? " stale" : ""}`}
        title={
          worst
            ? `${worst.label}: ${Math.round(worst.win.pct)}% used${
                worst.win.resetsAt ? `, resets in ${resetLabel(worst.win)}` : ""
              }`
            : "Usage window reset. Updates on the next turn."
        }
        aria-label="Usage"
        
        onClick={toggle}
      >
        <Ring pct={worst?.win.pct ?? 0} tone={worst?.t ?? ""} />
      </button>

      {open && (
        <div className={`um-pop${closing ? " closing" : ""}`}>
          {claude && <Section block={claude} note={claude.note} />}
          {claude && sum?.stale && <div className="um-stale">No agent has reported in a while.</div>}
          {codex && <Section block={codex} note={codexNote} />}
        </div>
      )}
    </div>
  );
}
