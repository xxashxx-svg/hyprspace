// Titlebar chip + popover for live provider usage. Claude pushes its numbers at us (see
// src/stores/usage.ts); codex has to be polled, and only while a codex pane is actually open.
import { useEffect, useMemo, useRef, useState } from "react";
import { useCodexUsage, useLiveUsage, toCodexBlock, toLiveBlock, type ProviderBlock } from "../stores/usage";
import { expired, resetLabel, tone, useLimits } from "../lib/limits";
import { useWorkspaces } from "../stores/workspace";
import { claudeLiveUsage, codexLiveUsage, providerUsageOne } from "../api";
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

const RANK: Record<string, number> = { "": 0, warn: 1, crit: 2 };

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

// the most urgent tone among a provider's live windows, for the dot on its tab
function worstTone(block: ProviderBlock): string {
  return block.windows
    .filter((w) => !expired(w.win))
    .map((w) => tone(w.win.pct, w.win))
    .reduce((a, b) => (RANK[b] > RANK[a] ? b : a), "");
}

/** one provider: a header, then a row per window it reports. Under a tab, the tab names it,
 *  so the header keeps only the plan. */
function Section({ block, note, tabbed }: { block: ProviderBlock; note?: string; tabbed?: boolean }) {
  return (
    <section className="um-card" style={{ "--brand": PROVIDER_COLOR[block.id] ?? "var(--text-1)" } as React.CSSProperties}>
      {!tabbed ? (
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
      ) : (
        block.plan && (
          <header className="um-head slim">
            <span className="um-plan" title={block.plan}>
              {block.plan}
            </span>
          </header>
        )
      )}
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
  // a plain boolean, so this doesn't re-render on unrelated workspace churn
  const hasCodex = useWorkspaces((s) =>
    s.workspaces.some((w) => w.sessions.some((x) => x.provider === "codex")),
  );
  const [open, setOpen] = useState(false);
  const [tab, setTab] = useState<"claude" | "codex">("claude");
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

  const { claude, codex, codexNote, claudeStale } = useLimits();

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

  return (
    <div className="um" ref={ref}>
      <button
        className={`um-chip ${worst?.t ?? ""}${!worst || claudeStale ? " stale" : ""}`}
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
          {claude && codex && (
            <div className="um-tabs" role="tablist">
              {[claude, codex].map((b) => {
                const t = worstTone(b);
                return (
                  <button key={b.id} role="tab" aria-selected={tab === b.id} className={tab === b.id ? "on" : ""} onClick={() => setTab(b.id as "claude" | "codex")}>
                    {LOGO[b.id] && <img src={LOGO[b.id]} alt="" />}
                    {b.label}
                    {t && <i className={`um-tab-dot ${t}`} title="Close to a limit" />}
                  </button>
                );
              })}
            </div>
          )}
          {claude && (!codex || tab === "claude") && (
            <>
              <Section block={claude} note={claude.note} tabbed={!!codex} />
              {claudeStale && <div className="um-stale">No agent has reported in a while.</div>}
            </>
          )}
          {codex && (!claude || tab === "codex") && <Section block={codex} note={codexNote} tabbed={!!claude} />}
        </div>
      )}
    </div>
  );
}
