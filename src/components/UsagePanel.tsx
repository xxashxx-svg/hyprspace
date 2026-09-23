import { useEffect, useRef, useState, type CSSProperties } from "react";
import { RotateCw, TriangleAlert } from "lucide-react";
import { providerUsageOne, type ProviderUsage, type UsageDay, type UsageModel } from "../api";
import type { ProviderBlock, UsageWindow } from "../stores/usage";
import { expired, resetLabel, tone, useLimits } from "../lib/limits";
import { PROVIDER_COLOR, PROVIDER_LOGO } from "../lib/brand";
import { relTime } from "../lib/time";
import { Blurred } from "./Blurred";

// Settings → Usage, in two views. Limits is what the plan allows and how much is left, from the
// same live readings the titlebar meter polls, so opening this never fetches anything extra.
// Activity is what each agent has done, read from its own files on this machine.
type View = "limits" | "activity";

function fmt(n: number): string {
  if (n >= 1e9) return (n / 1e9).toFixed(1) + "B";
  if (n >= 1e6) return (n / 1e6).toFixed(1) + "M";
  if (n >= 1e3) return (n / 1e3).toFixed(1) + "k";
  return n.toLocaleString();
}

// the moment a window resets, as a clock time: "11:20 PM" today, "Fri 9:00 AM" further out
function resetsAt(ms: number): string {
  const d = new Date(ms);
  const time = d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  return ms - Date.now() < 20 * 3600_000 ? time : `${d.toLocaleDateString([], { weekday: "short" })} ${time}`;
}

// ---------------------------------------------------------------- limits

function LimitRow({ label, win }: { label: string; win: UsageWindow }) {
  const gone = expired(win);
  const used = Math.round(win.pct);
  const left = 100 - used;
  const t = tone(win.pct, win);
  return (
    <div className={`up-row ${t}`}>
      <div className="up-row-info">
        <span className="up-row-label">{label}</span>
        <span className="up-big">
          {gone ? (
            "—"
          ) : (
            <>
              <b>{left}%</b> left
            </>
          )}
        </span>
        <span className="up-row-sub">
          {gone
            ? "Window reset. Updates on the next turn."
            : win.resetsAt && used > 0
              ? `+${used}% back in ${resetLabel(win)}`
              : win.resetsAt
                ? `Full, resets in ${resetLabel(win)}`
                : `${used}% used`}
        </span>
      </div>
      <div className="up-track" title={gone ? undefined : `${used}% used`}>
        <i className={`up-fill${!gone && left > 0 && left < 100 ? " edge" : ""}`} style={{ width: `${gone ? 0 : left}%` }} />
        {!gone && <span className="up-used">{used}% used</span>}
        {!gone && win.resetsAt && (
          <span className="up-chip">
            <RotateCw size={10} strokeWidth={2.4} />
            {resetsAt(win.resetsAt)}
          </span>
        )}
      </div>
    </div>
  );
}

function ExtraRow({ extra }: { extra: NonNullable<ProviderBlock["extra"]> }) {
  const cur = extra.currency === "USD" ? "$" : "";
  const pct = Math.round(extra.percent);
  return (
    <div className="up-row">
      <div className="up-row-info">
        <span className="up-row-label">Extra usage</span>
        <span className="up-big">
          <b>
            {cur}
            {extra.used.toFixed(2)}
          </b>{" "}
          of {cur}
          {extra.limit.toFixed(2)}
        </span>
        <span className="up-row-sub">This month</span>
      </div>
      <div className="up-track spent">
        <i className={`up-fill${pct > 0 && pct < 100 ? " edge" : ""}`} style={{ width: `${pct}%` }} />
        <span className="up-used">{pct}% used</span>
      </div>
    </div>
  );
}

function LimitCard({ block, note }: { block: ProviderBlock; note?: string }) {
  return (
    <section className="up-card" style={{ "--brand": PROVIDER_COLOR[block.id] ?? "var(--accent)" } as CSSProperties}>
      <header className="up-head">
        <span className="up-mark">{PROVIDER_LOGO[block.id] && <img src={PROVIDER_LOGO[block.id]} alt="" />}</span>
        <span className="up-name">{block.id === "claude" ? "Claude" : block.label}</span>
        {block.plan && <span className="up-plan">{block.plan}</span>}
        {block.updatedAt && (
          <span className="up-age">{relTime(block.updatedAt) === "now" ? "Updated just now" : `Updated ${relTime(block.updatedAt)} ago`}</span>
        )}
      </header>
      <div className="up-rows">
        {block.windows.map(({ key, label, win }) => (
          <LimitRow key={key} label={label} win={win} />
        ))}
        {block.extra && <ExtraRow extra={block.extra} />}
      </div>
      {note && <div className="up-note">{note}</div>}
    </section>
  );
}

function Limits() {
  const { claude, codex, codexNote, claudeStale, claudeMissing, codexMissing } = useLimits();
  const cards: { block: ProviderBlock; note?: string }[] = [];
  if (claude) cards.push({ block: claude, note: claude.note ?? (claudeStale ? "No agent has reported in a while." : undefined) });
  if (codex) cards.push({ block: codex, note: codexNote });
  return (
    <>
      {claudeMissing && (
        <div className="up-callout">
          <TriangleAlert size={14} />
          <span>
            <b>Claude:</b> {claudeMissing}
          </span>
        </div>
      )}
      {codexMissing && (
        <div className="up-callout">
          <TriangleAlert size={14} />
          <span>
            <b>Codex:</b> {codexMissing}
          </span>
        </div>
      )}
      {cards.map((c) => (
        <LimitCard key={c.block.id} block={c.block} note={c.note} />
      ))}
      {!cards.length && !claudeMissing && !codexMissing && (
        <div className="up-empty">No limits yet. They show up here once Claude or Codex is signed in on this machine.</div>
      )}
      {cards.length > 0 && (
        <p className="up-foot">
          A bar turns amber or red when you're using it up faster than the window runs out. Gemini, OpenCode and Grok
          don't report plan limits, so they only show under Activity.
        </p>
      )}
    </>
  );
}

// ---------------------------------------------------------------- activity

// tokens = the headline per provider. It counts input + output only, the same definition Claude's
// own /usage stats use. Cache re-reads dwarf real work (often 100x or more) and cost a fraction as
// much, so they sit on their own line rather than swamping the bar.
function Tokens({ u }: { u: ProviderUsage }) {
  const real = u.inputTokens + u.outputTokens;
  const inPct = real ? (u.inputTokens / real) * 100 : 0;
  return (
    <div className="up-tokens">
      <div className="up-tokens-top">
        <span className="up-big">
          <b>{fmt(real)}</b> tokens
        </span>
        {u.tokensWindow && <span className="up-dim">{u.tokensWindow}</span>}
      </div>
      {real > 0 && (
        <>
          <div className="up-split" title={`${fmt(u.inputTokens)} in, ${fmt(u.outputTokens)} out`}>
            {u.inputTokens > 0 && <span className="up-seg in" style={{ width: `${inPct}%` }} />}
            {u.outputTokens > 0 && <span className="up-seg out" style={{ width: `${100 - inPct}%` }} />}
          </div>
          <div className="up-legend">
            <span>
              <i className="up-seg in" />
              {fmt(u.inputTokens)} in
            </span>
            <span>
              <i className="up-seg out" />
              {fmt(u.outputTokens)} out
            </span>
            {u.cacheTokens > 0 && (
              <span className="up-cache" title="Re-reading cached context costs a fraction of new input, so it isn't counted in the total">
                + {fmt(u.cacheTokens)} read from cache
              </span>
            )}
          </div>
        </>
      )}
    </div>
  );
}

// "claude-opus-4-8" -> "Opus 4.8", "claude-haiku-4-5-20251001" -> "Haiku 4.5", and the older
// version-first ids too: "claude-3-5-sonnet-20241022" -> "Sonnet 3.5". Other ids pass through.
function prettyModel(id: string): string {
  if (!id.startsWith("claude-")) return id;
  const parts = id.replace(/^claude-/, "").replace(/-\d{8}$/, "").split("-");
  const words = parts.filter((p) => !/^\d+$/.test(p));
  const nums = parts.filter((p) => /^\d+$/.test(p));
  const name = words.map((w) => w.charAt(0).toUpperCase() + w.slice(1)).join(" ") || id;
  return nums.length ? `${name} ${nums.join(".")}` : name;
}

function Models({ models }: { models: UsageModel[] }) {
  const real = (m: UsageModel) => m.inputTokens + m.outputTokens;
  const shown = [...models].sort((a, b) => real(b) - real(a)).slice(0, 6);
  const max = Math.max(1, ...shown.map(real));
  return (
    <div className="up-block">
      <div className="up-block-head">
        <span>By model</span>
        <span className="up-dim">all time, in and out</span>
      </div>
      {shown.map((m) => (
        <div key={m.model} className="up-model" title={`${m.model}: ${fmt(m.inputTokens)} in, ${fmt(m.outputTokens)} out, ${fmt(m.cacheTokens)} cache`}>
          <span className="up-model-name">{prettyModel(m.model)}</span>
          <span className="up-model-track">
            <i style={{ width: `${(real(m) / max) * 100}%` }} />
          </span>
          <span className="up-model-val">{fmt(real(m))}</span>
        </div>
      ))}
    </div>
  );
}

const pad = (n: number) => String(n).padStart(2, "0");
const dayKey = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
const shortDate = (d: Date) => d.toLocaleDateString([], { month: "short", day: "numeric" });

// One slot per day for the last 30, so two active days read as two bars on a calendar rather than
// two walls of colour. If nothing falls in the last 30 days (a stats file that hasn't caught up),
// the window ends on the latest day there is instead, and says so.
function Days({ days, unit }: { days: UsageDay[]; unit: string }) {
  const N = 30;
  const byDate = new Map(days.map((d) => [d.date, d.value]));
  const build = (end: Date) =>
    Array.from({ length: N }, (_, i) => {
      const d = new Date(end);
      d.setDate(end.getDate() - (N - 1 - i));
      return { key: dayKey(d), date: d, value: byDate.get(dayKey(d)) ?? 0 };
    });
  const today = new Date();
  let slots = build(today);
  let endsToday = true;
  if (!slots.some((s) => s.value > 0)) {
    const last = days.map((d) => d.date).sort().pop();
    if (!last) return null;
    slots = build(new Date(`${last}T12:00:00`));
    endsToday = false;
  }
  const active = slots.filter((s) => s.value > 0);
  if (!active.length) return null;
  const max = Math.max(...active.map((s) => s.value));
  const peak = active.reduce((a, b) => (b.value > a.value ? b : a));
  return (
    <div className="up-block">
      <div className="up-block-head">
        <span>{endsToday ? "Last 30 days" : `30 days to ${shortDate(slots[N - 1].date)}`}</span>
        <span className="up-dim">
          {active.length} active {active.length === 1 ? "day" : "days"}, peak {fmt(peak.value)} {unit} on {shortDate(peak.date)}
        </span>
      </div>
      <div className="up-days">
        {slots.map((s) => (
          <span
            key={s.key}
            className={s.value ? (s === peak ? "peak" : "on") : ""}
            style={s.value ? { height: `${Math.max(10, (s.value / max) * 100)}%` } : undefined}
            title={s.value ? `${shortDate(s.date)}: ${s.value.toLocaleString()} ${unit}` : `${shortDate(s.date)}: nothing`}
          />
        ))}
      </div>
      <div className="up-days-axis">
        <span>{shortDate(slots[0].date)}</span>
        <span>{endsToday ? "Today" : shortDate(slots[N - 1].date)}</span>
      </div>
    </div>
  );
}

function ActivityCard({ u }: { u: ProviderUsage }) {
  const counts = [
    { label: "Sessions", n: u.sessions },
    { label: "Messages", n: u.messages },
    { label: "Tool calls", n: u.toolCalls },
    { label: "Active days", n: u.activeDays },
  ].filter((c) => c.n > 0);
  const hasBody = u.totalTokens > 0 || counts.length > 0 || u.daily.length > 1 || u.models.length > 0;
  return (
    <section className="up-card" style={{ "--brand": PROVIDER_COLOR[u.id] ?? "var(--accent)" } as CSSProperties}>
      <header className="up-head">
        <span className="up-mark">{PROVIDER_LOGO[u.id] && <img src={PROVIDER_LOGO[u.id]} alt="" />}</span>
        <span className="up-name">{u.label}</span>
        {u.plan && <span className="up-plan">{u.plan}</span>}
        {u.account && (
          <span className="up-age">
            <Blurred text={u.account} />
          </span>
        )}
      </header>
      {hasBody && (
        <div className="up-rows">
          {u.totalTokens > 0 && <Tokens u={u} />}
          {counts.length > 0 && (
            <div className="up-metrics">
              {counts.map((c) => (
                <div key={c.label} className="up-metric">
                  <span>{c.label}</span>
                  <b>{c.n.toLocaleString()}</b>
                </div>
              ))}
            </div>
          )}
          {u.models.length > 0 && <Models models={u.models} />}
          {u.daily.length > 1 && <Days days={u.daily} unit={u.dailyUnit ?? "msgs"} />}
        </div>
      )}
      {u.note && <div className="up-note">{u.note}</div>}
    </section>
  );
}

// scans stream in per provider, so the slow one (claude, with a long history) doesn't hold up the rest
const PROVIDERS = [
  { id: "claude", label: "Claude Code" },
  { id: "codex", label: "Codex" },
  { id: "gemini", label: "Gemini" },
  { id: "opencode", label: "OpenCode" },
  { id: "grok", label: "Grok" },
];

function Skeleton({ id, label, i }: { id: string; label: string; i: number }) {
  return (
    <section className="up-card up-skel" style={{ "--sk-delay": `${i * -0.18}s` } as CSSProperties}>
      <header className="up-head">
        <span className="up-mark">{PROVIDER_LOGO[id] && <img src={PROVIDER_LOGO[id]} alt="" />}</span>
        <span className="up-name">{label}</span>
        <span className="up-age up-reading">{id === "claude" ? "Reading its history. A long one takes a while." : "Reading local files"}</span>
      </header>
      <div className="up-rows">
        <span className="up-sk wide" />
        <span className="up-sk" />
      </div>
    </section>
  );
}

function Activity({ cards, pending }: { cards: Record<string, ProviderUsage>; pending: Set<string> }) {
  const data = PROVIDERS.map((p) => cards[p.id]).filter(Boolean);
  const on = data.filter((p) => p.signedIn);
  const off = data.filter((p) => !p.signedIn);
  const tokens = on.reduce((n, p) => n + p.inputTokens + p.outputTokens, 0);
  const sessions = on.reduce((n, p) => n + p.sessions, 0);
  return (
    <>
      {on.length > 0 && (
        <div className="up-strip">
          <div className="up-stat">
            <span className="up-stat-label">Tokens</span>
            <b>{fmt(tokens)}</b>
            <span className="up-stat-foot">In and out, recent</span>
          </div>
          <div className="up-stat">
            <span className="up-stat-label">Sessions</span>
            <b>{sessions.toLocaleString()}</b>
            <span className="up-stat-foot">
              Across {on.length} {on.length === 1 ? "agent" : "agents"}
            </span>
          </div>
          <div className="up-stat">
            <span className="up-stat-label">Signed in</span>
            <b>
              {on.length} <em>of {PROVIDERS.length}</em>
            </b>
            <span className="up-stat-logos">
              {PROVIDERS.map((p) => {
                const live = !!cards[p.id]?.signedIn;
                return PROVIDER_LOGO[p.id] ? (
                  <img key={p.id} src={PROVIDER_LOGO[p.id]} alt="" className={live ? "" : "off"} title={live ? p.label : `${p.label}, not signed in`} />
                ) : null;
              })}
            </span>
          </div>
        </div>
      )}
      {PROVIDERS.map((p, i) => {
        const u = cards[p.id];
        if (u?.signedIn) return <ActivityCard key={p.id} u={u} />;
        if (!u && pending.has(p.id)) return <Skeleton key={p.id} id={p.id} label={p.label} i={i} />;
        return null;
      })}
      {off.length > 0 && (
        <p className="up-foot">
          Not signed in on this machine: {off.map((p) => p.label).join(", ")}.
        </p>
      )}
      {data.length > 0 && <p className="up-foot">Everything here comes from each tool's own files on this machine. No network calls, no tokens spent.</p>}
    </>
  );
}

// ---------------------------------------------------------------- panel

export function UsagePanel() {
  const [view, setView] = useState<View>("limits");
  const [cards, setCards] = useState<Record<string, ProviderUsage>>({});
  const [pending, setPending] = useState<Set<string>>(new Set());
  const [, setTick] = useState(0);
  const genRef = useRef(0); // load generation: a stale scan still in flight must not clobber a newer one
  const loadedRef = useRef(false);

  // fire all five scans at once and let each card land on its own
  const load = () => {
    loadedRef.current = true;
    const gen = ++genRef.current;
    setPending(new Set(PROVIDERS.map((p) => p.id)));
    for (const p of PROVIDERS) {
      providerUsageOne(p.id)
        .then((u) => {
          if (genRef.current === gen && u) setCards((c) => ({ ...c, [p.id]: u }));
        })
        .catch(() => {})
        .finally(() => {
          if (genRef.current !== gen) return; // a newer load owns the pending set now
          setPending((s) => {
            const next = new Set(s);
            next.delete(p.id);
            return next;
          });
        });
    }
  };
  // the file scans are the slow part, so they wait until Activity is actually opened
  useEffect(() => {
    if (view === "activity" && !loadedRef.current) load();
  }, [view]);
  // countdowns move on their own, so re-render every 30s
  useEffect(() => {
    const t = setInterval(() => setTick((n) => n + 1), 30_000);
    return () => clearInterval(t);
  }, []);

  const loading = pending.size > 0;
  return (
    <div className="up">
      <div className="up-top">
        <div className="seg up-seg" role="tablist" aria-label="Usage view">
          {(["limits", "activity"] as const).map((v) => (
            <button key={v} role="tab" aria-selected={view === v} className={`seg-btn${view === v ? " active" : ""}`} onClick={() => setView(v)}>
              {v === "limits" ? "Limits" : "Activity"}
            </button>
          ))}
        </div>
        <span className="up-dim">
          {view === "limits" ? "Refreshes every 3 minutes" : loading ? "Reading local files" : "From each agent's own files"}
        </span>
        {view === "activity" && (
          <button className="btn" onClick={load} disabled={loading}>
            <RotateCw size={13} className={loading ? "up-spin" : ""} /> Refresh
          </button>
        )}
      </div>
      {view === "limits" ? <Limits /> : <Activity cards={cards} pending={pending} />}
    </div>
  );
}
