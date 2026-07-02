import { useEffect, useState } from "react";
import { useAutoAnimate } from "@formkit/auto-animate/react";
import {
  Repeat,
  Plus,
  Play,
  Square,
  Pause,
  Settings2,
  FolderGit2,
  Loader2,
  Pencil,
  Copy,
  Trash2,
} from "lucide-react";
import { useLoops, newLoop, loopRunId } from "../stores/loops";
import { useUi } from "../stores/ui";
import { useWorkspaces } from "../stores/workspace";
import { startLoop, stopLoop, pauseLoop, revealLoopWorktree } from "../lib/loops";
import { secretHas, revealPath } from "../api";
import { relTime } from "../lib/time";
import { LoopRunView } from "./LoopRunView";
import { LoopTerminal } from "./LoopTerminal";
import { LoopsManager } from "./LoopsManager";

// "1m 30s" / "45s" / "2h 05m"
function fmtDur(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000));
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ${String(s % 60).padStart(2, "0")}s`;
  return `${Math.floor(m / 60)}h ${String(m % 60).padStart(2, "0")}m`;
}

// past runs for one automation — persisted, so it survives restarts
function LoopHistory({ id }: { id: string }) {
  const hist = useLoops((s) => s.history[id]);
  if (!hist?.length) return null;
  return (
    <div className="loop-history">
      <div className="loop-history-head">Past runs</div>
      {hist.slice(0, 15).map((h) => (
        <div key={h.id} className="loop-history-row" title={h.lastResult || undefined}>
          <span className={`loop-dot s-${h.status}`} />
          <span className="loop-history-when">{relTime(h.endedAt)}</span>
          <span className="loop-history-meta">
            {h.iterations} iter · {fmtDur(h.endedAt - h.startedAt)}
            {h.costUsed ? ` · $${h.costUsed.toFixed(2)}` : ""}
          </span>
          {h.filesChanged ? (
            <span className="loop-history-diff">
              {h.filesChanged} file{h.filesChanged === 1 ? "" : "s"} · +{h.additions} −{h.deletions}
            </span>
          ) : null}
          <span className="loop-history-note">{h.note || h.lastResult || ""}</span>
          {h.worktreePath && (
            <button
              className="loop-history-open"
              title={`Open the run's worktree:\n${h.worktreePath}`}
              onClick={() => void revealPath(h.worktreePath!).catch(() => {})}
            >
              <FolderGit2 size={11} />
            </button>
          )}
        </div>
      ))}
    </div>
  );
}

const STATUS_LABEL: Record<string, string> = {
  idle: "idle",
  running: "running",
  paused: "paused",
  stopped: "stopped",
  done: "done",
  error: "error",
  crashloop: "stopped · no progress",
};

// The dedicated Loops page. Two faces: "Runs" — a master-detail of loops with the live agentic
// transcript on the right; and "Manage" — the classic editor (templates, API key, full config cards).
export function LoopsPage() {
  const loops = useLoops((s) => s.loops);
  const runs = useLoops((s) => s.runs);
  const tab = useUi((s) => s.loopsTab);
  const openLoopId = useUi((s) => s.openLoopId);
  const ids = Object.keys(loops);
  const [listRef] = useAutoAnimate();
  const [loopMenu, setLoopMenu] = useState<{ x: number; y: number; id: string } | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);

  const [hasKey, setHasKey] = useState(true);
  useEffect(() => {
    void secretHas("anthropic").then(setHasKey).catch(() => {});
  }, [tab]);

  const activeCwd = useWorkspaces((s) => {
    const w = s.workspaces.find((x) => x.id === s.activeId);
    return w && w.kind !== "open" ? w.cwd : "";
  });

  // running/paused loops float up so the live ones lead the list
  const rank = (id: string) => {
    const st = runs[id]?.status;
    return st === "running" || st === "paused" ? 0 : 1;
  };
  const list = Object.values(loops).sort((a, b) => rank(a.id) - rank(b.id));

  // selection: the explicitly opened loop, else the first running one, else the first loop
  const sel =
    (openLoopId && loops[openLoopId] && openLoopId) ||
    ids.find((i) => rank(i) === 0) ||
    ids[0] ||
    null;

  const addLoop = () => {
    const def = newLoop(activeCwd || "");
    def.name = "New automation";
    useLoops.getState().upsert(def);
    useUi.getState().focusLoop(def.id);
    useUi.getState().setLoopsTab("manage"); // drop into config to fill it in
  };

  const def = sel ? loops[sel] : null;
  const run = sel ? runs[sel] : undefined;
  const status = run?.status ?? "idle";
  const active = status === "running" || status === "paused";
  // interactive (pane) claude runs on the logged-in CLI — no API key needed (matches the engine)
  const needsKey = !!def && def.provider === "claude" && def.run !== "pane" && !hasKey;
  const canStart = !!def && !!def.prompt.trim() && !!def.folder && !needsKey;
  const startHint = !def
    ? ""
    : needsKey
      ? "Add an Anthropic API key in Manage first"
      : !def.prompt.trim() || !def.folder
        ? "Set a prompt and folder in Manage first"
        : "Run";

  return (
    <div className="loops-page">
      <div className={`loops-page-inner${tab === "runs" ? " wide" : ""}`}>
        <div className="loops-page-head">
          <Repeat size={20} strokeWidth={1.75} />
          <div>
            <h1>Automations</h1>
            <p>Agents that run on a schedule, on an interval, or until the job's done.</p>
          </div>
          <div className="loops-tabs">
            <button
              className={`loops-tab${tab === "runs" ? " active" : ""}`}
              onClick={() => useUi.getState().setLoopsTab("runs")}
            >
              Runs
            </button>
            <button
              className={`loops-tab${tab === "manage" ? " active" : ""}`}
              onClick={() => useUi.getState().setLoopsTab("manage")}
            >
              Manage
            </button>
          </div>
        </div>

        {tab === "manage" ? (
          <LoopsManager />
        ) : ids.length === 0 ? (
          <div className="loops-empty-runs">
            <p>No automations yet — create one to start automating.</p>
            <button className="btn" onClick={() => useUi.getState().setLoopsTab("manage")}>
              <Plus size={14} /> Create an automation
            </button>
          </div>
        ) : (
          <div className="loops-split">
            <div className="loops-master" ref={listRef}>
              {list.map((d) => {
                const r = runs[d.id];
                const st = r?.status ?? "idle";
                if (editingId === d.id) {
                  return (
                    <div key={d.id} className="loops-master-item editing">
                      <span className={`loop-dot s-${st}`} />
                      <input
                        className="rail-rename"
                        autoFocus
                        defaultValue={d.name}
                        onClick={(e) => e.stopPropagation()}
                        onBlur={(e) => {
                          const v = e.currentTarget.value.trim();
                          if (v) useLoops.getState().upsert({ ...d, name: v });
                          setEditingId(null);
                        }}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") e.currentTarget.blur();
                          if (e.key === "Escape") setEditingId(null);
                        }}
                      />
                    </div>
                  );
                }
                return (
                  <button
                    key={d.id}
                    className={`loops-master-item${sel === d.id ? " active" : ""}`}
                    title={d.name || "Untitled loop"}
                    onClick={() => useUi.getState().focusLoop(d.id)}
                    onDoubleClick={() => setEditingId(d.id)}
                    onContextMenu={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      setLoopMenu({ x: e.clientX, y: e.clientY, id: d.id });
                    }}
                  >
                    <span className={`loop-dot s-${st}`} />
                    <span className="loops-master-name">{d.name || "Untitled loop"}</span>
                    {st === "running" && <Loader2 size={12} className="rail-loop-spin" />}
                    <span className="rail-loop-count">
                      {r?.iteration ?? 0}/{d.stop.maxIterations}
                    </span>
                  </button>
                );
              })}
              <button className="loops-master-add" onClick={addLoop}>
                <Plus size={14} /> New automation
              </button>
            </div>

            <div className="loops-detail">
              {def && sel ? (
                <>
                  <div className="loops-detail-head">
                    <span className={`loop-dot s-${status}`} />
                    <span className="loops-detail-name">{def.name || "Untitled loop"}</span>
                    <span className={`loop-status-badge s-${status}`}>{STATUS_LABEL[status] ?? status}</span>
                    <div className="loops-detail-actions">
                      {active ? (
                        <>
                          {def.run !== "pane" && (
                            <button
                              className="svc-run"
                              title={status === "paused" ? "Resume" : "Pause"}
                              onClick={() => pauseLoop(sel, status !== "paused")}
                            >
                              <Pause size={13} />
                            </button>
                          )}
                          <button className="svc-stop" title="Stop" onClick={() => stopLoop(sel)}>
                            <Square size={13} />
                          </button>
                        </>
                      ) : (
                        <button className="svc-run" title={startHint} disabled={!canStart} onClick={() => startLoop(sel)}>
                          <Play size={13} />
                        </button>
                      )}
                      {run?.worktreePath && (
                        <button
                          className="loop-review"
                          title={`Open the isolated worktree:\n${run.worktreePath}`}
                          onClick={() => revealLoopWorktree(sel)}
                        >
                          <FolderGit2 size={12} /> Review changes
                        </button>
                      )}
                      <button
                        className="svc-run"
                        title="Edit configuration"
                        onClick={() => useUi.getState().setLoopsTab("manage")}
                      >
                        <Settings2 size={13} />
                      </button>
                    </div>
                  </div>
                  {def.run === "pane" ? (
                    active ? (
                      run?.nextRunAt ? (
                        <div className="loop-run-empty">
                          Scheduled — the interactive session launches at the next fire (while
                          HyprSpace is open).
                        </div>
                      ) : (
                        <LoopTerminal id={loopRunId(sel)} />
                      )
                    ) : (
                      <div className="loop-run-empty">
                        Interactive terminal automation — press ▶ to launch the session here.
                      </div>
                    )
                  ) : (
                    <LoopRunView id={sel} />
                  )}
                  <LoopHistory id={sel} />
                </>
              ) : (
                <div className="loop-run-empty">Select a loop to see its activity.</div>
              )}
            </div>
          </div>
        )}
      </div>

      {loopMenu &&
        (() => {
          const d = loops[loopMenu.id];
          if (!d) return null;
          const st = runs[loopMenu.id]?.status;
          const isActive = st === "running" || st === "paused";
          const close = () => setLoopMenu(null);
          return (
            <>
              <div
                className="ctx-backdrop"
                onClick={close}
                onContextMenu={(e) => {
                  e.preventDefault();
                  close();
                }}
              />
              <div className="ctx-menu" style={{ left: loopMenu.x, top: loopMenu.y }}>
                {isActive ? (
                  <button className="ctx-item" onClick={() => { stopLoop(loopMenu.id); close(); }}>
                    <Square size={14} />
                    <span>Stop loop</span>
                  </button>
                ) : (
                  <button
                    className="ctx-item"
                    onClick={() => {
                      startLoop(loopMenu.id);
                      useUi.getState().focusLoop(loopMenu.id);
                      close();
                    }}
                  >
                    <Play size={14} />
                    <span>Run loop</span>
                  </button>
                )}
                <button
                  className="ctx-item"
                  onClick={() => {
                    useUi.getState().focusLoop(loopMenu.id);
                    useUi.getState().setLoopsTab("manage");
                    close();
                  }}
                >
                  <Settings2 size={14} />
                  <span>Edit settings</span>
                </button>
                <button className="ctx-item" onClick={() => { setEditingId(loopMenu.id); close(); }}>
                  <Pencil size={14} />
                  <span>Rename</span>
                </button>
                <button
                  className="ctx-item"
                  onClick={() => {
                    useLoops.getState().upsert({
                      ...d,
                      id: crypto.randomUUID(),
                      name: `${d.name || "Untitled loop"} copy`,
                      enabled: false,
                    });
                    close();
                  }}
                >
                  <Copy size={14} />
                  <span>Duplicate</span>
                </button>
                <div className="ctx-sep" />
                <button
                  className="ctx-item danger"
                  onClick={() => {
                    if (isActive) stopLoop(loopMenu.id);
                    useLoops.getState().remove(loopMenu.id);
                    close();
                  }}
                >
                  <Trash2 size={14} />
                  <span>Delete</span>
                </button>
              </div>
            </>
          );
        })()}
    </div>
  );
}
