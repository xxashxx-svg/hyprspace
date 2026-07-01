import { useEffect, useState } from "react";
import { providerUsage, type ProviderUsage, type UsageWindow, type UsageDay } from "../api";
import { RotateCw } from "lucide-react";
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

// tokens = the headline metric per provider: total + how it splits across input / output / cache
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
          {fmt(u.totalTokens)} <em>tokens</em>
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

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="usage-stat">
      <span className="usage-stat-val">{value}</span>
      <span className="usage-stat-label">{label}</span>
    </div>
  );
}

function Sparkline({ days }: { days: UsageDay[] }) {
  const max = Math.max(1, ...days.map((d) => d.value));
  const peak = days.reduce((a, b) => (b.value > a.value ? b : a), days[0]);
  return (
    <div className="usage-activity">
      <div className="usage-activity-top">
        <span>Recent activity</span>
        <span className="usage-activity-peak">peak {fmt(peak?.value ?? 0)} msgs</span>
      </div>
      <div className="usage-spark">
        {days.map((d, i) => (
          <span
            key={i}
            className="usage-spark-bar"
            data-peak={d.value === max && max > 1}
            style={{ height: `${Math.max(6, Math.round((d.value / max) * 100))}%` }}
            title={`${d.date}: ${d.value.toLocaleString()} messages`}
          />
        ))}
      </div>
    </div>
  );
}

function UsageCard({ u }: { u: ProviderUsage }) {
  const hasCounts = u.sessions > 0 || u.messages > 0 || u.toolCalls > 0 || u.activeDays > 0;
  const hasBody =
    u.signedIn && (u.primary || u.secondary || u.totalTokens > 0 || hasCounts || u.daily.length > 1);
  return (
    <div className={`usage-card${u.signedIn ? "" : " off"}`}>
      <div className="usage-card-head">
        <span className="usage-ico">{LOGO[u.id] && <img src={LOGO[u.id]} alt="" />}</span>
        <div className="usage-card-title">
          <span className="usage-name">{u.label}</span>
          {u.account ? (
            <span className="usage-acct">{u.account}</span>
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

          {hasCounts && (
            <div className="usage-stats">
              {u.sessions > 0 && <Stat label="Sessions" value={u.sessions.toLocaleString()} />}
              {u.messages > 0 && <Stat label="Messages" value={u.messages.toLocaleString()} />}
              {u.toolCalls > 0 && <Stat label="Tool calls" value={u.toolCalls.toLocaleString()} />}
              {u.activeDays > 0 && <Stat label="Active days" value={u.activeDays.toLocaleString()} />}
            </div>
          )}

          {u.daily.length > 1 && <Sparkline days={u.daily} />}
        </div>
      )}

      {u.note && <div className="usage-note">{u.note}</div>}
    </div>
  );
}

function Skeleton() {
  return (
    <div className="usage-card usage-skeleton">
      <div className="usage-card-head">
        <span className="sk sk-ico" />
        <div className="usage-card-title">
          <span className="sk sk-line" style={{ width: 96 }} />
          <span className="sk sk-line" style={{ width: 140 }} />
        </div>
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

export function UsagePanel() {
  const [data, setData] = useState<ProviderUsage[] | null>(null);
  const [loading, setLoading] = useState(false);

  const load = () => {
    setLoading(true);
    providerUsage()
      .then(setData)
      .catch(() => setData([]))
      .finally(() => setLoading(false));
  };
  useEffect(load, []);

  const connected = data?.filter((p) => p.signedIn).length ?? 0;

  return (
    <div className="usage">
      <div className="usage-bar">
        <span className="usage-summary">
          {data ? `${connected} of ${data.length} providers connected` : "Reading usage…"}
        </span>
        <button className="btn" onClick={load} disabled={loading}>
          <RotateCw size={13} className={loading ? "usage-spin" : ""} /> Refresh
        </button>
      </div>

      {!data && loading && (
        <>
          <Skeleton />
          <Skeleton />
        </>
      )}
      {data && data.length === 0 && <div className="usage-empty">No provider usage found.</div>}
      {data && data.map((p) => <UsageCard key={p.id} u={p} />)}

      {data && data.length > 0 && (
        <div className="usage-foot">
          Everything here is read from each tool's own files on this machine — no network calls, no
          tokens used.
        </div>
      )}
    </div>
  );
}
