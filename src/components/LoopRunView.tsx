import { useEffect, useRef, useState, type ComponentType } from "react";
import { useLoops, type LoopEvent } from "../stores/loops";
import { stopLoop, pauseLoop } from "../lib/loops";
import {
  Terminal,
  FilePen,
  FilePlus,
  FileText,
  Search,
  FolderSearch,
  Globe,
  Wrench,
  Check,
  X,
  Loader2,
  ChevronRight,
  Brain,
  Square,
  Pause,
  Play,
} from "lucide-react";

// raw tool name → friendly label + icon, the way the agentic transcript reads it
type IconType = ComponentType<{ size?: number }>;
const TOOL_META: Record<string, { label: string; icon: IconType }> = {
  Bash: { label: "Terminal", icon: Terminal },
  Edit: { label: "Edit file", icon: FilePen },
  MultiEdit: { label: "Edit file", icon: FilePen },
  Write: { label: "Write file", icon: FilePlus },
  Read: { label: "Read file", icon: FileText },
  Grep: { label: "Search files", icon: Search },
  Glob: { label: "Find files", icon: FolderSearch },
  WebFetch: { label: "Fetch", icon: Globe },
  WebSearch: { label: "Web search", icon: Search },
};
const toolMeta = (name?: string) => (name && TOOL_META[name]) || { label: name || "Tool", icon: Wrench };

function fmtDur(ms?: number): string {
  if (ms == null) return "";
  const s = ms / 1000;
  return s < 10 ? `${s.toFixed(1)}s` : `${Math.round(s)}s`;
}

// compact token count: 980 → "980", 12345 → "12.3k", 1500000 → "1.5M"
function fmtTokens(n: number): string {
  if (n < 1000) return String(n);
  if (n < 1_000_000) return `${(n / 1000).toFixed(1)}k`;
  return `${(n / 1_000_000).toFixed(1)}M`;
}

// last path segment so a long path reads as just "service.ts" but the title keeps the full thing
const tail = (p: string) => p.split(/[\\/]/).pop() || p;

function ToolRow({ ev }: { ev: LoopEvent }) {
  const [open, setOpen] = useState(false);
  const { label, icon: Icon } = toolMeta(ev.tool);
  const isPath = ev.tool !== "Bash" && /[\\/]/.test(ev.arg ?? "");
  const shown = ev.arg ? (isPath ? tail(ev.arg) : ev.arg) : "";
  return (
    <>
      <button className="loop-ev loop-ev-tool" onClick={() => ev.arg && setOpen((o) => !o)} title={ev.arg}>
        <span className={`loop-ev-stat s-${ev.status ?? "running"}`}>
          {ev.status === "ok" ? <Check size={13} /> : ev.status === "error" ? <X size={13} /> : <Loader2 size={13} className="spin" />}
        </span>
        <span className="loop-ev-ico">
          <Icon size={13} />
        </span>
        <span className="loop-ev-label">{label}</span>
        <span className="loop-ev-arg">{shown}</span>
        {ev.durationMs != null && <span className="loop-ev-dur">{fmtDur(ev.durationMs)}</span>}
        {ev.arg && <ChevronRight size={13} className={`loop-ev-twist${open ? " open" : ""}`} />}
      </button>
      {open && ev.arg && <pre className="loop-ev-detail">{ev.arg}</pre>}
    </>
  );
}

function EventRow({ ev }: { ev: LoopEvent }) {
  if (ev.kind === "iteration")
    return (
      <div className="loop-ev-iter">
        <span>Iteration {ev.iteration}</span>
      </div>
    );
  if (ev.kind === "tool") return <ToolRow ev={ev} />;
  if (ev.kind === "thinking")
    return (
      <div className="loop-ev loop-ev-think">
        <Brain size={13} className="loop-ev-ico" />
        <span className="loop-ev-think-text">{ev.text}</span>
      </div>
    );
  if (ev.kind === "result")
    return (
      <div className="loop-ev loop-ev-result">
        <Check size={13} />
        <span>done{ev.arg ? ` ${ev.arg}` : ""}</span>
      </div>
    );
  // plain assistant / system text
  return <div className="loop-ev loop-ev-text">{ev.text}</div>;
}

// The live agentic transcript for one loop: tool calls with durations, thinking, results, plus the
// docked "Loop agent running" card while it's active. Reads structured events the engine emits.
export function LoopRunView({ id }: { id: string }) {
  const def = useLoops((s) => s.loops[id]);
  const run = useLoops((s) => s.runs[id]);
  const events = run?.events ?? [];

  const scrollRef = useRef<HTMLDivElement>(null);
  const stick = useRef(true);
  useEffect(() => {
    stick.current = true; // a freshly opened loop starts pinned to the latest activity
  }, [id]);
  useEffect(() => {
    const el = scrollRef.current;
    if (el && stick.current) el.scrollTop = el.scrollHeight;
  }, [events.length, id]);
  const onScroll = () => {
    const el = scrollRef.current;
    if (el) stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 48;
  };

  if (!def) return null;
  const status = run?.status ?? "idle";
  const active = status === "running" || status === "paused";
  const paused = status === "paused";

  return (
    <div className="loop-run">
      <div className="loop-run-scroll" ref={scrollRef} onScroll={onScroll}>
        {events.length === 0 ? (
          <div className="loop-run-empty">No activity yet. Start the loop to watch it work.</div>
        ) : (
          events.map((ev) => <EventRow key={ev.id} ev={ev} />)
        )}
      </div>

      {active && (
        <div className="loop-run-card">
          <span className="loop-run-card-ico">
            {paused ? <Pause size={16} /> : <Loader2 size={16} className="spin" />}
          </span>
          <div className="loop-run-card-text">
            <div className="loop-run-card-title">{paused ? "Loop paused" : "Loop agent running"}</div>
            <div className="loop-run-card-sub">
              {def.name || "Loop"} — iteration {run?.iteration ?? 0} / {def.stop.maxIterations}
              {run?.costUsed ? ` · $${run.costUsed.toFixed(3)}` : ""}
              {run?.tokensUsed ? ` · ${fmtTokens(run.tokensUsed)} tok` : ""}
              {def.stop.tokenBudget ? ` / ${fmtTokens(def.stop.tokenBudget)} budget` : ""}
            </div>
          </div>
          <button
            className="loop-run-resume"
            title={paused ? "Resume" : "Pause"}
            onClick={() => pauseLoop(id, !paused)}
          >
            {paused ? <Play size={13} /> : <Pause size={13} />}
          </button>
          <button className="loop-run-stop" onClick={() => stopLoop(id)}>
            <Square size={13} /> Stop loop
          </button>
        </div>
      )}
    </div>
  );
}
