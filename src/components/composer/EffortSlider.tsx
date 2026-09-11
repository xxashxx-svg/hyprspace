import { useEffect, useRef, type CSSProperties, type RefObject } from "react";
import { createPortal } from "react-dom";
import { Terminal as TerminalIcon } from "lucide-react";
import { EFFORT_LABEL, type ProviderId } from "../../lib/models";
import { PROVIDER_COLOR, PROVIDER_LOGO } from "../../lib/brand";

interface Props {
  open: boolean;
  anchorRef: RefObject<HTMLElement | null>;
  provider: ProviderId;
  modelLabel: string;
  levels: string[]; // the CLI's effort levels, lowest first
  value: string; // "" = the CLI's default
  onChange: (level: string) => void;
  onClose: () => void;
  /** one line on what a level means for the current model */
  noteFor: (level: string) => string;
}

/**
 * A slider for the effort level, in the agent's own colors. The leftmost stop is the CLI's
 * default; the rest are its levels, lowest to highest. Snaps to the stops. The visible thumb is
 * drawn by us so it can glide; the native range input underneath stays for input and focus.
 */
export function EffortSlider({ open, anchorRef, provider, modelLabel, levels, value, onChange, onClose, noteFor }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const stops = ["", ...levels];
  const idx = Math.max(0, stops.indexOf(value));
  const pct = stops.length > 1 ? (idx / (stops.length - 1)) * 100 : 0;
  const brand = PROVIDER_COLOR[provider] ?? "var(--accent)";
  const brand2 = PROVIDER_COLOR[`${provider}2`] ?? brand;

  useEffect(() => {
    const el = ref.current;
    const a = anchorRef.current;
    if (!open || !el || !a) return;
    const r = a.getBoundingClientRect();
    el.style.left = `${Math.max(8, Math.min(r.left, window.innerWidth - el.offsetWidth - 8))}px`;
    el.style.bottom = `${window.innerHeight - r.top + 8}px`;
  }, [open, anchorRef]);

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

  if (!open) return null;
  return createPortal(
    <div
      className="effort-pop"
      ref={ref}
      style={{ "--brand": brand, "--brand2": brand2, "--effort-pct": `${pct}%`, "--effort-n": stops.length } as CSSProperties}
    >
      <div className="effort-head">
        <span className="effort-mark">
          {PROVIDER_LOGO[provider] ? <img src={PROVIDER_LOGO[provider]} alt="" /> : <TerminalIcon size={13} />}
        </span>
        <span className="effort-model">{modelLabel}</span>
        {/* keyed so the name and blurb animate in on every change */}
        <span key={value} className="effort-name">
          {value ? EFFORT_LABEL[value] ?? value : "Default"}
        </span>
      </div>
      <div key={`b-${value}`} className="effort-blurb">
        {noteFor(value)}
      </div>

      <div className="effort-track">
        <span className="effort-fill">
          <span className="effort-stars" />
          <span className="effort-stars far" />
        </span>
        <span key={`f-${value}`} className="effort-flash" />
        <div className="effort-stops">
          {stops.map((s, i) => (
            <span key={s || "default"} className={`effort-stop${i <= idx ? " lit" : ""}${i === idx ? " here" : ""}`} />
          ))}
        </div>
        <span className="effort-thumb" />
        <input
          type="range"
          min={0}
          max={stops.length - 1}
          step={1}
          value={idx}
          autoFocus
          aria-label="Effort"
          onChange={(e) => onChange(stops[Number(e.target.value)] ?? "")}
        />
      </div>

      <div className="effort-ends">
        <button className={`effort-tick${idx === 0 ? " on" : ""}`} onClick={() => onChange("")}>
          Default
        </button>
        <button className={`effort-tick${idx === stops.length - 1 ? " on" : ""}`} onClick={() => onChange(stops[stops.length - 1])}>
          {EFFORT_LABEL[stops[stops.length - 1]] ?? stops[stops.length - 1]}
        </button>
      </div>
    </div>,
    document.body,
  );
}
