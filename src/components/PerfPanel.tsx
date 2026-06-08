import { useEffect, useRef, useState } from "react";
import { createPty, writePty, killPty } from "../api";
import "../styles/perf.css";

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

// collects PTY output + lets the runner await "next byte" / "quiet" windows
class Probe {
  total = 0;
  firstAt = 0;
  lastAt = 0;
  private waiters: (() => void)[] = [];
  push(b: Uint8Array) {
    const now = performance.now();
    if (!this.firstAt) this.firstAt = now;
    this.lastAt = now;
    this.total += b.length;
    const w = this.waiters;
    this.waiters = [];
    w.forEach((fn) => fn());
  }
  reset() {
    this.total = 0;
    this.firstAt = 0;
    this.lastAt = 0;
  }
  waitNext(timeoutMs = 400): Promise<boolean> {
    return new Promise((res) => {
      let done = false;
      const fire = (v: boolean) => {
        if (!done) {
          done = true;
          res(v);
        }
      };
      this.waiters.push(() => fire(true));
      setTimeout(() => fire(false), timeoutMs);
    });
  }
  async quiet(ms: number, maxWait = 5000) {
    const start = performance.now();
    while (performance.now() - start < maxWait) {
      const before = this.lastAt;
      await sleep(ms);
      if (this.lastAt === before) return;
    }
  }
}

export function PerfPanel({ onClose }: { onClose: () => void }) {
  const [lines, setLines] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const add = (s: string) => setLines((l) => [...l, s]);
  const setLast = (s: string) => setLines((l) => [...l.slice(0, -1), s]);

  // take keyboard focus AWAY from the terminal behind the modal so keystrokes
  // (esp. Ctrl+C) can't leak into a running pane while this is open.
  useEffect(() => {
    (document.activeElement as HTMLElement | null)?.blur?.();
  }, []);

  const run = async () => {
    if (busyRef.current) return;
    busyRef.current = true;
    setBusy(true);
    setLines([]);
    const id = "perf-" + crypto.randomUUID();
    const probe = new Probe();
    const enc = new TextEncoder();
    try {
      add("› spawning throwaway shell…");
      await createPty({ id, cwd: "", args: [], cols: 120, rows: 40 }, { onData: (b) => probe.push(b) });
      await probe.quiet(140); // let the prompt settle

      // ---- echo round-trip latency (input → echo through the full pipeline) ----
      add("  measuring echo… 0");
      const lat: number[] = [];
      const ITERS = 28;
      for (let i = 0; i < ITERS; i++) {
        await probe.quiet(18);
        const c = String.fromCharCode(97 + (i % 26));
        const t0 = performance.now();
        void writePty(id, enc.encode(c));
        const ok = await probe.waitNext(400);
        if (ok && i >= 3) lat.push(performance.now() - t0); // skip 3 warmup samples
        setLast(`  measuring echo… ${i + 1}/${ITERS}`);
      }
      void writePty(id, enc.encode("\x15")); // Ctrl+U: clear the probe input line
      await probe.quiet(60);
      if (lat.length) {
        lat.sort((a, b) => a - b);
        const med = lat[lat.length >> 1];
        const p99 = lat[Math.min(lat.length - 1, Math.floor(lat.length * 0.99))];
        setLast(`✓ echo round-trip: min ${lat[0].toFixed(1)} · median ${med.toFixed(1)} · p99 ${p99.toFixed(1)} ms  (n=${lat.length})`);
      } else setLast("✗ echo: no samples");

      // ---- throughput (firehose) ----
      add("  measuring throughput…");
      await probe.quiet(120);
      probe.reset();
      void writePty(id, enc.encode("1..500000\r"));
      await probe.quiet(400); // until output stops
      const dur = (probe.lastAt - probe.firstAt) / 1000;
      const mb = probe.total / 1048576;
      if (dur > 0.01) {
        setLast(`✓ throughput: ${mb.toFixed(1)} MB in ${dur.toFixed(2)} s = ${(mb / dur).toFixed(1)} MB/s`);
      } else setLast("✗ throughput: stream too short to measure");
      add("— done (echo excludes monitor vsync; it's the controllable input→echo path)");
    } catch (e) {
      add("✗ error: " + e);
    } finally {
      void killPty(id);
      busyRef.current = false;
      setBusy(false);
    }
  };

  return (
    <div className="perf-overlay" onMouseDown={onClose}>
      <div className="perf-card" onMouseDown={(e) => e.stopPropagation()}>
        <div className="perf-head">
          <span>⚡ Terminal perf — real numbers (WebView2 channel + Rust PTY)</span>
          <button className="perf-x" onClick={onClose} aria-label="Close">
            ×
          </button>
        </div>
        <div className="perf-body">
          {lines.length === 0 && (
            <div className="perf-hint">
              Probes a throwaway shell for echo round-trip latency + firehose throughput through the real
              pipeline. Keyboard is parked here while open, so it won't touch your panes.
            </div>
          )}
          {lines.map((l, i) => (
            <div key={i} className="perf-line">
              {l}
            </div>
          ))}
        </div>
        <div className="perf-foot">
          <button className="btn primary" disabled={busy} onClick={run}>
            {busy ? "running…" : "Run benchmark"}
          </button>
        </div>
      </div>
    </div>
  );
}
