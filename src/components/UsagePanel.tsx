import { useEffect, useRef, useState, type CSSProperties } from "react";
import {
  providerUsageOne,
  type ProviderUsage,
  type UsageWindow,
  type UsageDay,
  type UsageModel,
} from "../api";
import { RotateCw, TriangleAlert } from "lucide-react";
import { Blurred } from "./Blurred";
import claudeLogo from "../assets/brand/claude.svg";
import geminiLogo from "../assets/brand/gemini.svg";
import openaiLogo from "../assets/brand/openai.svg";
import opencodeLogo from "../assets/brand/opencode.svg";
import grokLogo from "../assets/brand/grok.svg";

// same brand marks the launcher / providers use (Codex = OpenAI)
const LOGO: Record<string, string> = {
  claude: claudeLogo,
  codex: openaiLogo,
  gemini: geminiLogo,
  opencode: opencodeLogo,
  grok: grokLogo,
};

function fmt(n: number): string {
  if (n >= 1e9) return (n / 1e9).toFixed(1) + "B";
  if (n >= 1e6) return (n / 1e6).toFixed(1) + "M";
  if (n >= 1e3) return (n / 1e3).toFixed(1) + "k";
  return n.toLocaleString();
}

// turn a rolling-window length into a friendly label
function windowLabel(min: number): string {
  if (!min) return "Limit";
  if (min <= 60) return `${min}m window`;
  const h = min / 60;
  if (h <= 23) return `${Math.round(h)}h window`;
  const d = h / 24;
  if (Math.abs(d - 7) < 0.6) return "Weekly limit";
  return `${Math.round(d)}-day window`;
}

function resetLabel(resetsAt: number): string {
  if (!resetsAt) return "";
  const secs = resetsAt - Math.floor(Date.now() / 1000);
  if (secs <= 0) return "resets soon";
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (h >= 24) return `resets in ${Math.floor(h / 24)}d ${h % 24}h`;
  if (h > 0) return `resets in ${h}h ${m}m`;
  return `resets in ${m}m`;
}

function LimitBar({ w }: { w: UsageWindow }) {
  const pct = Math.max(0, Math.min(100, w.usedPercent));
  return (
    <div className="usage-limit">
      <div className="usage-limit-top">
        <span className="usage-limit-label">{windowLabel(w.windowMinutes)}</span>
        <span className="usage-limit-val">
          <b data-hot={pct >= 80}>{pct.toFixed(0)}%</b>
          {w.resetsAt ? ` · ${resetLabel(w.resetsAt)}` : ""}
        </span>
      </div>
      <div className="usage-limit-track">
        <div className="usage-limit-fill" style={{ width: `${pct}%` }} data-hot={pct >= 80} />
      </div>
    </div>
  );
}

// tokens = the headline metric per provider. The headline counts input + output only — the same
// definition Claude's own /usage stats use — since cache re-reads dwarf real work by ~100x and
// would make the number meaningless. The bar + legend still show the full split including cache.
function Tokens({ u }: { u: ProviderUsage }) {
  const total = Math.max(1, u.totalTokens);
  const parts = [
    { key: "in", n: u.inputTokens, label: "in" },
    { key: "out", n: u.outputTokens, label: "out" },
    { key: "cache", n: u.cacheTokens, label: "cache" },
  ].filter((p) => p.n > 0);
  return (
    <div className="usage-tokens">
      <div className="usage-tokens-top">
        <span className="usage-tokens-val">
          {fmt(u.inputTokens + u.outputTokens)} <em>tokens</em>
        </span>
        {u.tokensWindow && <span className="usage-tokens-win">{u.tokensWindow}</span>}
      </div>
      <div className="usage-tokens-bar">
        {parts.map((p) => (
          <span key={p.key} className={`usage-seg ${p.key}`} style={{ width: `${(p.n / total) * 100}%` }} />
        ))}
      </div>
      <div className="usage-tokens-legend">
        {parts.map((p) => (
          <span key={p.key} className="usage-leg">
            <i className={`usage-dot ${p.key}`} />
            {fmt(p.n)} {p.label}
          </span>
        ))}
      </div>
    </div>
  );
}

function Stat({ label, value, sub, hot }: { label: string; value: string; sub?: string; hot?: boolean }) {
  return (
    <div className="usage-stat">
      <span className="usage-stat-val" data-hot={!!hot}>
        {value}
      </span>
      <span className="usage-stat-label">{label}</span>
      {sub && <span className="usage-stat-sub">{sub}</span>}
    </div>
  );
}

function Sparkline({ days, unit }: { days: UsageDay[]; unit: string }) {
  const max = Math.max(1, ...days.map((d) => d.value));
  const peak = days.reduce((a, b) => (b.value > a.value ? b : a), days[0]);
  return (
    <div className="usage-activity">
      <div className="usage-activity-top">
        <span>Recent activity</span>
        <span className="usage-activity-peak">
          peak {fmt(peak?.value ?? 0)} {unit}
        </span>
      </div>
      <div className="usage-spark">
        {days.map((d, i) => (
          <span
            key={i}
            className="usage-spark-bar"
            data-peak={d.value === max && max > 1}
            style={{ height: `${Math.max(6, Math.round((d.value / max) * 100))}%` }}
            title={`${d.date}: ${d.value.toLocaleString()} ${unit}`}
          />
        ))}
      </div>
    </div>
  );
}

// "claude-opus-4-8" -> "Opus 4.8", "claude-haiku-4-5-20251001" -> "Haiku 4.5",
// and legacy version-first ids too: "claude-3-5-sonnet-20241022" -> "Sonnet 3.5".
// non-claude ids (e.g. "<synthetic>") pass through raw.
function prettyModel(id: string): string {
  if (!id.startsWith("claude-")) return id;
  const parts = id.replace(/^claude-/, "").replace(/-\d{8}$/, "").split("-");
  const words = parts.filter((p) => !/^\d+$/.test(p));
  const nums = parts.filter((p) => /^\d+$/.test(p));
  const name = words.map((w) => w.charAt(0).toUpperCase() + w.slice(1)).join(" ") || id;
  return nums.length ? `${name} ${nums.join(".")}` : name;
}

// lifetime magnitude per model — single hue, direct value labels. in + out only (Claude's own
// /usage definition); the full split incl. cache lives in each row's tooltip.
function Models({ models }: { models: UsageModel[] }) {
  const real = (m: UsageModel) => m.inputTokens + m.outputTokens;
  const shown = [...models].sort((a, b) => real(b) - real(a)).slice(0, 6);
  const max = Math.max(1, ...shown.map(real));
  return (
    <div className="usage-models">
      <div className="usage-activity-top">
        <span>By model</span>
        <span className="usage-activity-peak">all time · in+out</span>
      </div>
      {shown.map((m) => (
        <div
          key={m.model}
          className="usage-model"
          title={`${m.model}: ${fmt(m.inputTokens)} in · ${fmt(m.outputTokens)} out · ${fmt(m.cacheTokens)} cache`}
        >
          <span className="usage-model-name">{prettyModel(m.model)}</span>
          <span className="usage-model-track">
            <i style={{ width: `${(real(m) / max) * 100}%` }} />
          </span>
          <span className="usage-model-val">{fmt(real(m))}</span>
        </div>
      ))}
    </div>
  );
}

// every rolling window across every provider, hottest first
function hotWindows(data: ProviderUsage[]): { p: ProviderUsage; w: UsageWindow }[] {
  const all: { p: ProviderUsage; w: UsageWindow }[] = [];
  for (const p of data) {
    if (!p.signedIn) continue;
    for (const w of [p.primary, p.secondary]) if (w) all.push({ p, w });
  }
  return all.sort((a, b) => b.w.usedPercent - a.w.usedPercent);
}

function Overview({ data }: { data: ProviderUsage[] }) {
  const on = data.filter((p) => p.signedIn);
  // in + out only, matching the per-card headline (cache re-reads would swamp it)
  const tokens = on.reduce((n, p) => n + p.inputTokens + p.outputTokens, 0);
  const sessions = on.reduce((n, p) => n + p.sessions, 0);
  const hot = hotWindows(data)[0];
  return (
    <div className="usage-overview">
      <Stat label="Tokens · recent" value={fmt(tokens)} />
      <Stat label="Sessions" value={sessions.toLocaleString()} />
      {hot ? (
        <Stat
          label={`${hot.p.label} · ${windowLabel(hot.w.windowMinutes).toLowerCase()}`}
          value={`${Math.max(0, Math.min(100, hot.w.usedPercent)).toFixed(0)}%`}
          sub={resetLabel(hot.w.resetsAt)}
          hot={hot.w.usedPercent >= 80}
        />
      ) : (
        <Stat label="Tracked limits" value="—" />
      )}
      <Stat label="Connected" value={`${on.length}/${data.length}`} />
    </div>
  );
}

function Alerts({ data }: { data: ProviderUsage[] }) {
  const hot = hotWindows(data).filter((x) => x.w.usedPercent >= 80);
  if (!hot.length) return null;
  return (
    <>
      {hot.map((x, i) => (
        <div key={i} className="usage-alert">
          <TriangleAlert size={14} />
          <b>{x.p.label}</b>
          <span>
            {windowLabel(x.w.windowMinutes).toLowerCase()} at {x.w.usedPercent.toFixed(0)}%
            {x.w.resetsAt ? ` · ${resetLabel(x.w.resetsAt)}` : ""}
          </span>
        </div>
      ))}
    </>
  );
}

function UsageCard({ u }: { u: ProviderUsage }) {
  const hasCounts = u.sessions > 0 || u.messages > 0 || u.toolCalls > 0 || u.activeDays > 0;
  const hasBody =
    u.signedIn &&
    (u.primary || u.secondary || u.totalTokens > 0 || hasCounts || u.daily.length > 1 || u.models.length > 0);
  return (
    <div className={`usage-card${u.signedIn ? "" : " off"}`}>
      <div className="usage-card-head">
        <span className="usage-ico">{LOGO[u.id] && <img src={LOGO[u.id]} alt="" />}</span>
        <div className="usage-card-title">
          <span className="usage-name">{u.label}</span>
          {u.account ? (
            <span className="usage-acct">
              <Blurred text={u.account} />
            </span>
          ) : (
            !u.signedIn && <span className="usage-acct">Not connected</span>
          )}
        </div>
        <div className="usage-badges">
          {u.plan && <span className="usage-badge">{u.plan}</span>}
          {u.tier && <span className="usage-badge dim">{u.tier} tier</span>}
        </div>
      </div>

      {hasBody && (
        <div className="usage-body">
          {(u.primary || u.secondary) && (
            <div className="usage-limits">
              {u.primary && <LimitBar w={u.primary} />}
              {u.secondary && <LimitBar w={u.secondary} />}
            </div>
          )}

          {u.totalTokens > 0 && <Tokens u={u} />}

          {u.models.length > 0 && <Models models={u.models} />}

          {hasCounts && (
            <div className="usage-stats">
              {u.sessions > 0 && <Stat label="Sessions" value={u.sessions.toLocaleString()} />}
              {u.messages > 0 && <Stat label="Messages" value={u.messages.toLocaleString()} />}
              {u.toolCalls > 0 && <Stat label="Tool calls" value={u.toolCalls.toLocaleString()} />}
              {u.activeDays > 0 && <Stat label="Active days" value={u.activeDays.toLocaleString()} />}
            </div>
          )}

          {u.daily.length > 1 && <Sparkline days={u.daily} unit={u.dailyUnit ?? "msgs"} />}
        </div>
      )}

      {u.note && <div className="usage-note">{u.note}</div>}
    </div>
  );
}

// scans stream in per provider — the slow one (claude) shouldn't hold up the rest
const PROVIDERS = [
  { id: "claude", label: "Claude Code" },
  { id: "codex", label: "Codex" },
  { id: "gemini", label: "Gemini" },
  { id: "opencode", label: "OpenCode" },
  { id: "grok", label: "Grok" },
];

// skeleton with the real brand + name, so the panel feels "there" while it fills in
function ProviderSkeleton({ id, label, i }: { id: string; label: string; i: number }) {
  return (
    <div className="usage-card usage-skeleton" style={{ "--sk-delay": `${i * -0.18}s` } as CSSProperties}>
      <div className="usage-card-head">
        <span className="usage-ico">{LOGO[id] && <img src={LOGO[id]} alt="" />}</span>
        <div className="usage-card-title">
          <span className="usage-name">{label}</span>
          <span className="usage-acct usage-reading">Reading local files…</span>
        </div>
        <span className="sk sk-badge" />
      </div>
      <div className="usage-body">
        <span className="sk sk-bar" />
        <div className="usage-stats">
          <span className="sk sk-tile" />
          <span className="sk sk-tile" />
          <span className="sk sk-tile" />
        </div>
      </div>
    </div>
  );
}

function OverviewSkeleton() {
  return (
    <div className="usage-overview">
      {[0, 1, 2, 3].map((i) => (
        <span key={i} className="sk sk-tile" style={{ "--sk-delay": `${i * -0.1}s` } as CSSProperties} />
      ))}
    </div>
  );
}

export function UsagePanel() {
  const [cards, setCards] = useState<Record<string, ProviderUsage>>({});
  const [pending, setPending] = useState<Set<string>>(new Set());
  const [, setTick] = useState(0);
  const genRef = useRef(0); // load generation — stale in-flight scans must not clobber newer ones

  // fire all five scans at once and let each card land on its own
  const load = () => {
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
  useEffect(load, []);
  // keep "resets in" countdowns fresh + re-scan every few minutes while the tab is open
  useEffect(() => {
    const tick = setInterval(() => setTick((n) => n + 1), 30_000);
    const auto = setInterval(load, 5 * 60_000);
    return () => {
      clearInterval(tick);
      clearInterval(auto);
    };
  }, []);

  const loading = pending.size > 0;
  const data = PROVIDERS.map((p) => cards[p.id]).filter(Boolean);
  const connected = data.filter((p) => p.signedIn).length;
  const reading = PROVIDERS.filter((p) => pending.has(p.id) && !cards[p.id]).map((p) => p.label);

  return (
    <div className="usage">
      <div className="usage-bar">
        <span className="usage-summary">
          {reading.length
            ? `Reading ${reading.join(", ")}…`
            : `${connected} of ${PROVIDERS.length} providers connected`}
        </span>
        <button className="btn" onClick={load} disabled={loading}>
          <RotateCw size={13} className={loading ? "usage-spin" : ""} /> Refresh
        </button>
      </div>

      {data.length > 0 ? (
        // never regress from data back to a skeleton — a provider that failed once would
        // otherwise flicker the whole strip on every refresh
        <>
          <Alerts data={data} />
          <Overview data={data} />
        </>
      ) : (
        loading && <OverviewSkeleton />
      )}

      {PROVIDERS.map((p, i) =>
        cards[p.id] ? (
          <UsageCard key={p.id} u={cards[p.id]} />
        ) : pending.has(p.id) ? (
          <ProviderSkeleton key={`sk-${p.id}`} id={p.id} label={p.label} i={i} />
        ) : null,
      )}

      {!loading && data.length === 0 && <div className="usage-empty">No provider usage found.</div>}

      {data.length > 0 && (
        <div className="usage-foot">
          Everything here is read from each tool's own files on this machine. No network calls, no
          tokens used.
        </div>
      )}
    </div>
  );
}
