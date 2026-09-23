import { useEffect, useState, type CSSProperties } from "react";
import { Bot, FilePenLine, Hand, ListChecks, RefreshCw, ShieldOff, type LucideIcon } from "lucide-react";
import { useSettings } from "../../stores/settings";
import { useProviders, catalogFor } from "../../stores/providers";
import {
  AGENT_IDS,
  CLAUDE_PERMISSIONS,
  CODEX_MODES,
  EFFORT_LABEL,
  GEMINI_MODES,
  effortsFor,
  type PermissionMode,
  type ProviderId,
} from "../../lib/models";
import { PROVIDER_COLOR, PROVIDER_LOGO, PROVIDER_NAME } from "../../lib/brand";
import { relTime } from "../../lib/time";
import { Blurred } from "../Blurred";

// Settings → Defaults: what each agent starts a session with. The model, how hard it thinks, and
// what it may do without asking. The composer can still change model and effort for one session.

// The default model for one agent: the catalog, plus a box for any other id.
function ModelSelect({ provider }: { provider: ProviderId }) {
  const value = useSettings((s) => s.agentModel[provider] ?? "");
  const set = useSettings((s) => s.setAgentModel);
  const codexModels = useProviders((p) => p.codexModels);
  const cat = catalogFor(provider, codexModels);
  const known = cat.models.some((m) => m.id === value);
  const [custom, setCustom] = useState(!known);
  return (
    <div className="df-model">
      <span className="df-select">
      <select
        className="set-select"
        value={custom ? "__custom" : value}
        onChange={(e) => {
          if (e.target.value === "__custom") {
            setCustom(true);
            return;
          }
          setCustom(false);
          set(provider, e.target.value);
        }}
      >
        {cat.models.map((m) => (
          <option key={m.id || "default"} value={m.id}>
            {m.label}
          </option>
        ))}
        {cat.customModel && <option value="__custom">Other…</option>}
      </select>
      </span>
      {custom && (
        <input
          className="set-input"
          autoFocus
          placeholder={cat.modelHint ?? "model id"}
          defaultValue={known ? "" : value}
          onBlur={(e) => set(provider, e.currentTarget.value.trim())}
          onKeyDown={(e) => {
            if (e.key === "Enter") e.currentTarget.blur();
          }}
        />
      )}
    </div>
  );
}

function EffortSelect({ provider, efforts }: { provider: ProviderId; efforts: string[] }) {
  const value = useSettings((s) => s.agentEffort[provider] ?? "");
  const set = useSettings((s) => s.setAgentEffort);
  return (
    <span className="df-select">
      <select className="set-select" value={value} onChange={(e) => set(provider, e.target.value)}>
        <option value="">Default</option>
        {efforts.map((lvl) => (
          <option key={lvl} value={lvl}>
            {EFFORT_LABEL[lvl] ?? lvl}
          </option>
        ))}
      </select>
    </span>
  );
}

const MODE_ICON: Record<string, LucideIcon> = {
  "Plan mode": ListChecks,
  "Ask each time": Hand,
  "Accept edits": FilePenLine,
  Agent: Bot,
};

function Permissions<T>({ modes, value, onChange }: { modes: PermissionMode<T>[]; value: T; onChange: (v: T) => void }) {
  const current = modes.find((m) => m.value === value) ?? modes[0];
  return (
    <div className="df-perm">
      <div className="df-label">Permissions</div>
      <div className="df-seg" role="radiogroup" aria-label="Permissions">
        {modes.map((m) => (
          <button
            key={String(m.value)}
            type="button"
            role="radio"
            aria-checked={m.value === value}
            className={`${m.value === value ? "on" : ""}${m.risky ? " risky" : ""}`}
            onClick={() => onChange(m.value)}
          >
            {(() => {
              const Icon = m.risky ? ShieldOff : (MODE_ICON[m.label] ?? Hand);
              return <Icon size={14} strokeWidth={2} />;
            })()}
            {m.label}
          </button>
        ))}
      </div>
      <div className={`df-says${current.risky ? " risky" : ""}`}>{current.says}</div>
    </div>
  );
}

function AgentCard({ id }: { id: ProviderId }) {
  const st = useProviders((p) => p.status[id]);
  const codexModels = useProviders((p) => p.codexModels);
  const s = useSettings();
  const efforts = effortsFor(id, s.agentModel[id] ?? "", catalogFor(id, codexModels));
  const state = !st ? "wait" : st.account ? "ok" : "warn";
  return (
    <section className="df-card" style={{ "--brand": PROVIDER_COLOR[id] ?? "var(--accent)" } as CSSProperties}>
      <header className="df-head">
        <span className="df-mark">{PROVIDER_LOGO[id] && <img src={PROVIDER_LOGO[id]} alt="" />}</span>
        <div className="df-title">
          <span className="df-name">{PROVIDER_NAME[id]}</span>
          <span className={`df-status ${state}`}>
            <i />
            {!st ? (
              "Checking"
            ) : st.account ? (
              <>
                Signed in as&nbsp;
                <Blurred text={st.account} />
              </>
            ) : (
              st.detail || "Installed"
            )}
          </span>
        </div>
        {st?.plan && <span className="df-plan">{st.plan}</span>}
        {st?.version && <span className="df-ver">v{st.version}</span>}
      </header>
      <div className="df-body">
        <div className="df-fields">
          <label className="df-field">
            <span className="df-label">Model</span>
            <ModelSelect provider={id} />
          </label>
          {efforts.length > 0 && (
            <label className="df-field">
              <span className="df-label">Effort</span>
              <EffortSelect provider={id} efforts={efforts} />
            </label>
          )}
        </div>
        {id === "claude" && <Permissions modes={CLAUDE_PERMISSIONS} value={s.claudePermission} onChange={s.setClaudePermission} />}
        {id === "codex" && <Permissions modes={CODEX_MODES} value={s.codexMode} onChange={s.setCodexMode} />}
        {id === "gemini" && <Permissions modes={GEMINI_MODES} value={s.geminiYolo} onChange={s.setGeminiYolo} />}
      </div>
    </section>
  );
}

export function Defaults() {
  const providers = useProviders((p) => p.status);
  const checking = useProviders((p) => p.checking);
  const checkedAt = useProviders((p) => p.checkedAt);
  const refresh = useProviders((p) => p.refresh);
  useEffect(() => {
    if (checkedAt == null) void refresh();
  }, [checkedAt, refresh]);
  // an agent whose CLI isn't installed has nothing to set, so it moves to one line at the bottom
  const shown = AGENT_IDS.filter((id) => !providers[id] || providers[id]?.installed);
  const missing = AGENT_IDS.filter((id) => providers[id] && !providers[id]?.installed);
  return (
    <div className="df">
      <div className="df-top">
        <span>{checkedAt ? `Checked ${relTime(checkedAt) === "now" ? "just now" : `${relTime(checkedAt)} ago`}` : "Checking which agents are installed"}</span>
        <button className="btn" disabled={checking} onClick={() => void refresh()}>
          <RefreshCw size={13} className={checking ? "df-spin" : ""} />
          Check again
        </button>
      </div>
      {shown.map((id) => (
        <AgentCard key={id} id={id} />
      ))}
      {missing.length > 0 && (
        <div className="df-missing">
          <span className="df-label">Not installed</span>
          <span className="df-chips">
            {missing.map((id) => (
              <span key={id} className="df-chip">
                {PROVIDER_LOGO[id] && <img src={PROVIDER_LOGO[id]} alt="" />}
                {PROVIDER_NAME[id]}
              </span>
            ))}
          </span>
          <span className="df-dim">Install one, then press Check again.</span>
        </div>
      )}
      <p className="df-foot">
        These apply to new sessions. The composer can change the model and effort for any one of them. Terminal and WSL
        open a plain shell, so they have nothing to set.
      </p>
    </div>
  );
}
