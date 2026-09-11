import { useEffect, useMemo, useRef, useState, type RefObject } from "react";
import { createPortal } from "react-dom";
import { Check, Search, Terminal as TerminalIcon } from "lucide-react";
import { useProviders, isInstalled, catalogFor } from "../../stores/providers";
import { AGENT_IDS, effortsFor, type ProviderId } from "../../lib/models";
import { PROVIDER_COLOR, PROVIDER_LOGO, PROVIDER_NAME, PROVIDER_DESC } from "../../lib/brand";
import { isWindows } from "../../platform";

export interface ModelChoice {
  provider: ProviderId;
  model: string;
  effort: string;
}

interface Props {
  open: boolean;
  anchorRef: RefObject<HTMLElement | null>;
  value: ModelChoice;
  onChange: (next: ModelChoice) => void;
  onClose: () => void;
}

const TABS: ProviderId[] = [...AGENT_IDS, ...(isWindows ? (["wsl"] as ProviderId[]) : []), "terminal"];

/**
 * Picks the agent and its model for a launch. Providers sit as tabs across the
 * top, models below. A provider whose CLI is missing stays visible but says so instead of listing
 * models, so the reason it cannot be picked is on screen.
 */
export function ModelPicker({ open, anchorRef, value, onChange, onClose }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const [tab, setTab] = useState<ProviderId>(value.provider);
  const [q, setQ] = useState("");
  const [custom, setCustom] = useState("");
  const status = useProviders((s) => s.status);
  const codexModels = useProviders((s) => s.codexModels);

  useEffect(() => {
    if (!open) return;
    setTab(value.provider);
    setQ("");
    setCustom("");
    inputRef.current?.focus();
  }, [open, value.provider]);

  // Positioned from the anchor before paint; opens upward so it never covers the text box below.
  useEffect(() => {
    const el = ref.current;
    const a = anchorRef.current;
    if (!open || !el || !a) return;
    const r = a.getBoundingClientRect();
    const left = Math.max(8, Math.min(r.left, window.innerWidth - el.offsetWidth - 8));
    el.style.left = `${left}px`;
    el.style.bottom = `${window.innerHeight - r.top + 6}px`;
    el.style.maxHeight = `${r.top - 14}px`;
  }, [open, anchorRef, tab]);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      const t = e.target as Node;
      if (ref.current?.contains(t) || anchorRef.current?.contains(t)) return;
      onClose();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open, anchorRef, onClose]);

  const cat = catalogFor(tab, codexModels);
  const installed = isInstalled(status, tab);
  const qq = q.trim().toLowerCase();
  const models = useMemo(
    () => cat.models.filter((m) => !qq || m.label.toLowerCase().includes(qq) || m.id.toLowerCase().includes(qq)),
    [cat, qq],
  );
  const isShell = tab === "terminal" || tab === "wsl";

  const pick = (model: string) => {
    // keep the effort when this model still supports it; otherwise back to the default
    const keep = tab === value.provider && effortsFor(tab, model, cat).includes(value.effort);
    onChange({ provider: tab, model, effort: keep ? value.effort : "" });
    onClose();
  };

  if (!open) return null;
  return createPortal(
    <div className="pop-menu mp" ref={ref}>
      <div className="mp-tabs">
        {TABS.map((id) => {
          const ok = isInstalled(status, id);
          return (
            <button
              key={id}
              className={`mp-tab${tab === id ? " on" : ""}${ok ? "" : " off"}`}
              style={{ "--brand": PROVIDER_COLOR[id] ?? "var(--accent)" } as React.CSSProperties}
              title={ok ? PROVIDER_NAME[id] : `${PROVIDER_NAME[id]} is not installed`}
              onClick={() => setTab(id)}
            >
              {PROVIDER_LOGO[id] ? <img src={PROVIDER_LOGO[id]} alt="" /> : <TerminalIcon size={14} />}
            </button>
          );
        })}
      </div>

      <div className="mp-head">
        <span className="mp-name">{PROVIDER_NAME[tab]}</span>
        <span className="mp-desc">{PROVIDER_DESC[tab]}</span>
        {status[tab as keyof typeof status]?.version && (
          <span className="mp-ver">v{status[tab as keyof typeof status]?.version}</span>
        )}
      </div>

      {!installed ? (
        <div className="mp-missing">
          <strong>{PROVIDER_NAME[tab]} is not installed.</strong>
          <span>
            Install the <code>{tab}</code> CLI, sign in to it once, then re-check in Settings.
          </span>
        </div>
      ) : isShell ? (
        <div className="pop-menu-list">
          <button className="pop-menu-item on" onClick={() => pick("")}>
            <TerminalIcon size={15} />
            <span className="pop-menu-text">
              <span className="pop-menu-title">{PROVIDER_NAME[tab]}</span>
              <span className="pop-menu-desc">{PROVIDER_DESC[tab]}. No model to choose.</span>
            </span>
            {value.provider === tab && <Check size={14} className="pop-menu-check" />}
          </button>
        </div>
      ) : (
        <>
          <div className="pop-menu-search">
            <Search size={13} />
            <input
              ref={inputRef}
              value={q}
              placeholder="Search models"
              spellCheck={false}
              onChange={(e) => setQ(e.target.value)}
            />
          </div>
          <div className="pop-menu-list">
            {models.map((m) => {
              const on = value.provider === tab && value.model === m.id;
              return (
                <button key={m.id || "default"} className={`pop-menu-item${on ? " on" : ""}`} onClick={() => pick(m.id)}>
                  <span className="pop-menu-text">
                    <span className="pop-menu-title">{m.label}</span>
                    <span className="pop-menu-desc">{m.note ?? m.id}</span>
                  </span>
                  {on && <Check size={14} className="pop-menu-check" />}
                </button>
              );
            })}
            {models.length === 0 && <div className="pop-menu-empty">No model matches.</div>}
            {cat.customModel && (
              <div className="mp-custom">
                <input
                  value={custom}
                  placeholder={`Custom model id, e.g. ${cat.modelHint ?? "model"}`}
                  spellCheck={false}
                  onChange={(e) => setCustom(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && custom.trim()) pick(custom.trim());
                  }}
                />
                <button disabled={!custom.trim()} onClick={() => pick(custom.trim())}>
                  Use
                </button>
              </div>
            )}
          </div>
        </>
      )}
    </div>,
    document.body,
  );
}
