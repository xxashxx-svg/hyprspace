// Titlebar chip + popover for live provider usage. Claude pushes its numbers at us (see
// src/stores/usage.ts); codex has to be polled, and only while a codex pane is actually open.
import { useEffect, useMemo, useRef, useState } from "react";
import {
  useUsage,
  useCodexUsage,
  summarize,
  toCodexBlock,
  type UsageWindow,
  type ProviderBlock,
} from "../stores/usage";
import { useWorkspaces } from "../stores/workspace";
import { providerUsageOne } from "../api";
import { relTime } from "../lib/time";
import claudeLogo from "../assets/brand/claude.svg";
import openaiLogo from "../assets/brand/openai.svg";

const LOGO: Record<string, string> = { claude: claudeLogo, codex: openaiLogo };

const CODEX_POLL_MS = 60_000;

// claude's own warning shape: it compares how much you've spent against how far through the window
// you are, not against a flat line. 89% with hours left is a problem; 89% with minutes left isn't.
// Pace needs the window's real length — judging a 7-day window against 5 hours makes everything
// look critical — so without a known length we fall back to flat thresholds.
function tone(pct: number, w?: UsageWindow): "" | "warn" | "crit" {
  if (expired(w)) return "";
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

function Bar({ pct, tone, tick }: { pct: number; tone: string; tick?: number }) {
  // the pace notch only says anything when there's spend to compare it against, and down at the
  // track's rounded ends it just reads as a stray speck — so keep it to the stretch where it means
  // something
  const showTick = tick !== undefined && pct > 0 && tick > 5 && tick < 95;
  return (
    <div className={`um-bar ${tone}`}>
      <i style={{ width: `${pct}%` }} />
      {showTick && (
        <span className="um-tick" style={{ left: `${tick}%` }} title="how far through this window you are" />
      )}
    </div>
  );
}

/** one provider's block: a header, then a bar per window it reports */
function Section({
  block,
  first,
  tick,
  note,
}: {
  block: ProviderBlock;
  first?: boolean;
  tick?: number;
  note?: string;
}) {
  return (
    <>
      <div className={`um-hdr${first ? " first" : ""}`}>
        {LOGO[block.id] && <img className="um-logo" src={LOGO[block.id]} alt="" />}
        <span className="um-name">{block.label}</span>
        {block.plan && (
          <span className="um-plan" title={block.plan}>
            {block.plan}
          </span>
        )}
      </div>
      {block.windows.map(({ key, label, win }) => {
        const gone = expired(win);
        const t = tone(win.pct, win);
        return (
          <div className="um-grp" key={key}>
            <div className="um-row">
              <span className="um-lbl">{label}</span>
              <span className={`um-val ${t}${gone ? " muted" : ""}`}>
                {gone ? "—" : `${Math.round(win.pct)}%`}
              </span>
            </div>
            <Bar pct={gone ? 0 : win.pct} tone={t} tick={gone || key !== "five_hour" ? undefined : tick} />
            {gone ? (
              <div className="um-sub stale">window reset · updates next turn</div>
            ) : (
              win.resetsAt && !note && <div className="um-sub">resets in {resetLabel(win)}</div>
            )}
          </div>
        );
      })}
      {note && <div className="um-sub stale">{note}</div>}
    </>
  );
}

export function UsageMeter() {
  const byPane = useUsage((s) => s.byPane);
  const codex = useCodexUsage((s) => s.codex);
  // a plain boolean, so this doesn't re-render on unrelated workspace churn
  const hasCodex = useWorkspaces((s) =>
    s.workspaces.some((w) => w.sessions.some((x) => x.provider === "codex")),
  );
  const [open, setOpen] = useState(false);
  const [, bump] = useState(0);
  const ref = useRef<HTMLDivElement>(null);

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

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const sum = useMemo(() => summarize(byPane), [byPane]);

  const claude: ProviderBlock | null = useMemo(() => {
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

  // the ring follows the most urgent window across every provider — otherwise it would sit calmly on
  // claude's session limit while codex was the thing about to run out.
  const worst = useMemo(() => {
    // an expired window tells us nothing, so it can't be the thing the ring reports
    const all = [...(claude?.windows ?? []), ...(codex?.windows ?? [])].filter((w) => !expired(w.win));
    if (!all.length) return null;
    return all
      .map((w) => ({ ...w, t: tone(w.win.pct, w.win) }))
      .reduce((a, b) =>
        RANK[b.t] > RANK[a.t] || (RANK[b.t] === RANK[a.t] && b.win.pct > a.win.pct) ? b : a,
      );
  }, [claude, codex]);

  const anyWindow = (claude?.windows.length ?? 0) + (codex?.windows.length ?? 0) > 0;
  if (!worst && !anyWindow) return null;

  const five = sum?.five;
  const elapsed =
    five?.resetsAt && five.windowMs && !expired(five)
      ? Math.max(0, Math.min(100, ((five.windowMs - (five.resetsAt - Date.now())) / five.windowMs) * 100))
      : undefined;

  // codex only records its windows mid-session, so a number can easily be weeks old — say so
  const codexAge = codex?.updatedAt ? relTime(codex.updatedAt) : undefined;
  const codexNote = codexAge ? (codexAge === "now" ? "just updated" : `as of ${codexAge} ago`) : undefined;

  return (
    <div className="um" ref={ref}>
      <button
        className={`um-chip ${worst?.t ?? ""}${!worst || sum?.stale ? " stale" : ""}`}
        title={
          worst
            ? `${worst.label} — ${Math.round(worst.win.pct)}% used${
                worst.win.resetsAt ? `, resets in ${resetLabel(worst.win)}` : ""
              }`
            : "Usage window reset — updates on the next turn"
        }
        onClick={() => setOpen((o) => !o)}
      >
        <Ring pct={worst?.win.pct ?? 0} tone={worst?.t ?? ""} />
      </button>

      {open && (
        <div className="um-pop">
          {claude && <Section block={claude} first tick={elapsed} />}
          {claude && sum?.stale && <div className="um-stale">no agent has reported in a while</div>}
          {codex && <Section block={codex} first={!claude} note={codexNote} />}
        </div>
      )}
    </div>
  );
}
