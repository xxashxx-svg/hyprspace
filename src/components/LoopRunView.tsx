import { useEffect, useRef } from "react";
import { useLoops } from "../stores/loops";
import { stopLoop, pauseLoop } from "../lib/automations";
import { Loader2, Clock, Square, Pause, Play } from "lucide-react";

// "next run today 14:30" / "next run Fri, Jul 3 03:00"
function fmtWhen(t: number): string {
  const d = new Date(t);
  const today = new Date().toDateString() === d.toDateString();
  const time = d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  return today
    ? `today ${time}`
    : `${d.toLocaleDateString(undefined, { weekday: "short", month: "short", day: "numeric" })} ${time}`;
}

// The run view for one automation: the engine's log lines, plus a docked card with pause/stop and
// wall-clock progress while it's active. The agent itself runs in a real pane — "Watch" jumps there.
export function LoopRunView({ id }: { id: string }) {
  const def = useLoops((s) => s.loops[id]);
  const run = useLoops((s) => s.runs[id]);
  const logs = run?.logs ?? [];

  const scrollRef = useRef<HTMLDivElement>(null);
  const stick = useRef(true);
  useEffect(() => {
    stick.current = true; // a freshly opened automation starts pinned to the latest activity
  }, [id]);
  useEffect(() => {
    const el = scrollRef.current;
    if (el && stick.current) el.scrollTop = el.scrollHeight;
  }, [logs.length, id]);
  const onScroll = () => {
    const el = scrollRef.current;
    if (el) stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 48;
  };

  if (!def) return null;
  const status = run?.status ?? "idle";
  const active = status === "running" || status === "paused";
  const paused = status === "paused";
  // one prompt, one session — so "progress" is wall-clock against the give-up budget
  const budgetMin = def.stop.timeBudgetMin && def.stop.timeBudgetMin > 0 ? def.stop.timeBudgetMin : 60;
  const elapsedMin = run?.startedAt ? (Date.now() - run.startedAt) / 60000 : 0;
  const pct = Math.max(3, Math.min(100, (elapsedMin / budgetMin) * 100));
  // armed-and-waiting (scheduled fire in the future) must NOT look like a hung run
  const nextAt = run?.nextRunAt;
  const waiting = active && !paused && !!nextAt && nextAt > Date.now();

  return (
    <div className="loop-run">
      <div className="loop-run-scroll" ref={scrollRef} onScroll={onScroll}>
        {logs.length > 0 ? (
          <div className="loop-run-logs">
            {logs.map((l, i) => (
              <div className="loop-run-logline" key={i}>
                {l}
              </div>
            ))}
          </div>
        ) : (
          <div className="loop-run-empty">
            {waiting && nextAt
              ? `Scheduled — next run ${fmtWhen(nextAt)}. It fires automatically while HyprSpace is open.`
              : active
                ? "Launching the agent — it runs in a tab in this project's space."
                : "No activity yet. Start the automation to watch it work."}
          </div>
        )}
      </div>

      {active && (
        <div className={`loop-run-card${paused ? " paused" : ""}`}>
          <div className="loop-run-card-row">
            <span className="loop-run-card-ico">
              {paused ? <Pause size={15} /> : waiting ? <Clock size={15} /> : <Loader2 size={15} className="spin" />}
            </span>
            <div className="loop-run-card-text">
              <div className="loop-run-card-title">
                {paused ? "Automation paused" : waiting ? "Scheduled" : "Automation running"}
              </div>
              <div className="loop-run-card-sub">
                <span className="loop-run-card-name">{def.name || "Automation"}</span>
                {waiting && nextAt ? (
                  <span className="loop-run-sep">next run {fmtWhen(nextAt)}</span>
                ) : (
                  <span className="loop-run-sep">
                    {Math.floor(elapsedMin)}m of {budgetMin}m
                  </span>
                )}
              </div>
            </div>
            <button
              className="loop-run-btn loop-run-btn-icon"
              title={paused ? "Resume" : "Pause"}
              onClick={() => pauseLoop(id, !paused)}
            >
              {paused ? <Play size={14} /> : <Pause size={14} />}
            </button>
            <button className="loop-run-btn loop-run-stop" title="Stop automation" onClick={() => stopLoop(id)}>
              <Square size={10} fill="currentColor" strokeWidth={0} />
              Stop
            </button>
          </div>
          <div className="loop-run-progress">
            <div className="loop-run-progress-fill" style={{ width: `${pct}%` }} />
          </div>
        </div>
      )}
    </div>
  );
}
