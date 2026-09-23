import { useEffect, useRef, useState, type CSSProperties, type RefObject } from "react";
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

// a label has to fit under its stop, so the long names get a short form
const SHORT: Record<string, string> = { minimal: "Min", medium: "Med", xhigh: "X-high" };
const full = (lvl: string) => (lvl ? EFFORT_LABEL[lvl] ?? lvl : "Default");
const short = (lvl: string) => SHORT[lvl] ?? full(lvl);

/**
 * A slider for the effort level, in the agent's own colors. The leftmost stop is the CLI's
 * default; the rest are its levels, lowest to highest, each with its name underneath. Snaps to
 * the stops. The visible track and thumb are drawn by us so they can glide; the native range
 * input on top stays for dragging, arrow keys and focus. Hovering a label previews it above.
 */
export function EffortSlider({ open, anchorRef, provider, modelLabel, levels, value, onChange, onClose, noteFor }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const [hover, setHover] = useState<string | null>(null);
  const stops = ["", ...levels];
  const idx = Math.max(0, stops.indexOf(value));
  const at = (i: number) => (stops.length > 1 ? (i / (stops.length - 1)) * 100 : 0);
  const shown = hover ?? value;
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
      style={{ "--brand": brand, "--brand2": brand2, "--effort-pct": `${at(idx)}%`, "--effort-n": stops.length } as CSSProperties}
    >
      <div className="effort-head">
        <span className="effort-mark">
          {PROVIDER_LOGO[provider] ? <img src={PROVIDER_LOGO[provider]} alt="" /> : <TerminalIcon size={13} />}
        </span>
        <span className="effort-model">{modelLabel}</span>
        {/* keyed so the name and blurb animate in on every change */}
        <span key={shown} className={`effort-name${hover != null && hover !== value ? " preview" : ""}`}>
          {full(shown)}
        </span>
      </div>
      <div key={`b-${shown}`} className="effort-blurb">
        {noteFor(shown)}
      </div>

      <div className="effort-rail">
        <div className="effort-line">
          <span className="effort-fill" />
          {stops.map((s, i) => (
            <span
              key={s || "default"}
              className={`effort-dot${i < idx ? " lit" : ""}${i === idx ? " here" : ""}`}
              style={{ left: `${at(i)}%` }}
            />
          ))}
          <span className="effort-thumb" />
        </div>
        <input
          type="range"
          min={0}
          max={stops.length - 1}
          step={1}
          value={idx}
          autoFocus
          aria-label="Effort"
          aria-valuetext={full(value)}
          onChange={(e) => onChange(stops[Number(e.target.value)] ?? "")}
          onKeyDown={(e) => e.key === "Enter" && onClose()}
        />
      </div>

      <div className="effort-labels" onMouseLeave={() => setHover(null)}>
        {stops.map((s, i) => (
          <button
            key={s || "default"}
            type="button"
            tabIndex={-1}
            className={`effort-lbl${i === idx ? " on" : ""}${i === 0 ? " first" : ""}${i === stops.length - 1 ? " last" : ""}`}
            style={{ left: `${at(i)}%` }}
            onMouseEnter={() => setHover(s)}
            onClick={() => onChange(s)}
          >
            {short(s)}
          </button>
        ))}
      </div>
    </div>,
    document.body,
  );
}
