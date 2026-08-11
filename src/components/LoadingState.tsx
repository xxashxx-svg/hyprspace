import { useEffect, useState } from "react";

/* ─────────────────────────────────────────────────────────
 * LOADING STATE — pixel-grid loader for long-running work
 *
 * Variants:
 *   drive  — square cells, chevron wavefront driving right;
 *            the 650ms cycle is shorter than the sweep, so
 *            two fronts are always in flight
 *   dots   — same wavefront, circular cells
 *   orbit  — a comet lapping the grid perimeter
 *
 * Paired with a shimmering label and a live elapsed timer
 * in mono tabular figures. Reduced motion freezes the grid
 * to its dim state; the timer still ticks.
 * ───────────────────────────────────────────────────────── */

// 3x3, delay rising with column + distance from the middle row → a chevron sweeping right
const chevron = Array.from({ length: 9 }, (_, i) => {
  const r = Math.floor(i / 3),
    c = i % 3;
  return (c + Math.abs(r - 1)) * 90;
});

const ORBIT_ORDER = [0, 1, 2, 5, 8, 7, 6, 3]; // perimeter, clockwise — the centre never lights
const orbit = Array.from({ length: 9 }, (_, i) => {
  const k = ORBIT_ORDER.indexOf(i);
  return k === -1 ? null : k * 110;
});

export type LoadingVariant = "drive" | "dots" | "orbit";

const PATTERNS: Record<LoadingVariant, { delays: (number | null)[]; dur: number; round: boolean }> = {
  drive: { delays: chevron, dur: 650, round: false },
  dots: { delays: chevron, dur: 650, round: true },
  orbit: { delays: orbit, dur: 950, round: false },
};

/** Elapsed since mount. Measured against a start timestamp rather than counting ticks: WebView2
 *  throttles timers in background windows, so a tick counter would silently under-report while a
 *  hidden pane's work kept running. The clock read happens in the interval, keeping render pure. */
function useElapsed(): string {
  const [ms, setMs] = useState(0);
  useEffect(() => {
    const start = Date.now();
    const t = setInterval(() => setMs(Date.now() - start), 100);
    return () => clearInterval(t);
  }, []);
  const total = ms / 1000;
  if (total < 60) return `${total.toFixed(1)}s`;
  return `${Math.floor(total / 60)}m ${(total % 60).toFixed(1)}s`;
}

export function LoadingState({
  label = "Churning",
  variant = "drive",
  timer = true,
}: {
  label?: string;
  variant?: LoadingVariant;
  /** hide the elapsed readout for short waits where a stopwatch just adds noise */
  timer?: boolean;
}) {
  const elapsed = useElapsed();
  const { delays, dur, round } = PATTERNS[variant] ?? PATTERNS.drive;

  return (
    <div className="loadstate" role="status" aria-live="polite">
      <span aria-hidden className="loadstate-grid">
        {delays.map((d, i) => (
          <span
            key={i}
            className={`loadstate-cell${round ? " round" : ""}${d === null ? " off" : ""}`}
            style={d === null ? undefined : { animationDuration: `${dur}ms`, animationDelay: `${d}ms` }}
          />
        ))}
      </span>
      <span className="loadstate-label">{label}</span>
      {timer && <span className="loadstate-time">{elapsed}</span>}
    </div>
  );
}
