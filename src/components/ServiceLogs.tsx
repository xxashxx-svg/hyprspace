import { memo, useEffect, useLayoutEffect, useRef } from "react";
import { useUi } from "../stores/ui";
import { useServices } from "../stores/services";
import { X, Square, Trash2 } from "lucide-react";

// strip ANSI escape sequences so the log reads as plain text
// eslint-disable-next-line no-control-regex
const ANSI = /\[[0-9;?]*[A-Za-z]/g;
const clean = (s: string) => s.replace(ANSI, "");

// memoized so unchanged lines don't re-run the ANSI strip every time a new batch lands
const LogLine = memo(function LogLine({ line }: { line: string }) {
  return <div className="log-line">{clean(line) || " "}</div>;
});

// Output viewer for a background service. Streams the captured stdout/stderr lines live.
export function ServiceLogs() {
  const target = useUi((s) => s.serviceLogsFor);
  const close = useUi((s) => s.closeServiceLogs);
  const id = target?.id ?? "";
  const lines = useServices((s) => (id ? s.logs[id] : undefined));
  const running = useServices((s) => (id ? id in s.running : false));
  const bodyRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!target) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [target, close]);

  // stick to the bottom as new lines arrive (unless the user scrolled up)
  useLayoutEffect(() => {
    const el = bodyRef.current;
    if (!el) return;
    const nearBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
    if (nearBottom) el.scrollTop = el.scrollHeight;
  }, [lines]);

  if (!target) return null;

  return (
    <div className="svc-overlay" onMouseDown={close}>
      <div className="log-dialog" onMouseDown={(e) => e.stopPropagation()}>
        <div className="log-head">
          <span className={`log-dot${running ? " on" : ""}`} />
          <span className="log-title" title={target.name}>
            {target.name}
          </span>
          <span className="log-state">{running ? "running" : "stopped"}</span>
          <div className="log-actions">
            {running && (
              <button
                className="log-btn"
                title="Stop"
                onClick={() => useServices.getState().stop(id)}
              >
                <Square size={13} />
              </button>
            )}
            <button
              className="log-btn"
              title="Clear output"
              onClick={() => useServices.getState().clearLogs(id)}
            >
              <Trash2 size={13} />
            </button>
            <button className="log-btn" title="Close (Esc)" onClick={close}>
              <X size={15} />
            </button>
          </div>
        </div>
        <div className="log-body" ref={bodyRef}>
          {!lines || lines.length === 0 ? (
            <div className="log-empty">No output yet…</div>
          ) : (
            lines.map((l, i) => <LogLine key={i} line={l} />)
          )}
        </div>
      </div>
    </div>
  );
}
