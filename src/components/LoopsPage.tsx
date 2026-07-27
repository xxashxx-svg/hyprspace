import { useState } from "react";
import { useShallow } from "zustand/react/shallow";
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
  Eye,
} from "lucide-react";
import { useLoops, newLoop } from "../stores/loops";
import { useUi } from "../stores/ui";
import { useWorkspaces } from "../stores/workspace";
import { startLoop, stopLoop, pauseLoop, revealLoopWorktree } from "../lib/automations";
import { revealPath } from "../api";
import { relTime } from "../lib/time";
import { LoopRunView } from "./LoopRunView";
import { AutomationEditor } from "./AutomationEditor";

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

// The dedicated Automations page: a master-detail of automations with the run view (or the
// editor) on the right.
export function LoopsPage() {
  const loops = useLoops((s) => s.loops);
  // `runs` re-mints on every log flush — the live view renders inside LoopRunView, so the page
  // itself only selects the coarse per-run bits it shows (status dots)
  const statuses = useLoops(
    useShallow((s) => {
      const m: Record<string, string> = {};
      for (const [k, r] of Object.entries(s.runs)) m[k] = r.status;
      return m;
    }),
  );
  const openLoopId = useUi((s) => s.openLoopId);
  const ids = Object.keys(loops);
  const [listRef] = useAutoAnimate();
  const [loopMenu, setLoopMenu] = useState<{ x: number; y: number; id: string } | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null); // inline rename in the list
  const [editing, setEditing] = useState(false); // the detail pane is showing the editor

  const activeCwd = useWorkspaces((s) => {
    const w = s.workspaces.find((x) => x.id === s.activeId);
    return w && w.kind !== "open" ? w.cwd : "";
  });

  // running/paused loops float up so the live ones lead the list
  const rank = (id: string) => {
    const st = statuses[id];
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
    setEditing(true); // drop straight into the editor to fill it in
  };

  const def = sel ? loops[sel] : null;
  // narrow per-field selects for the detail head — status/iteration change per iteration, not per line
  const worktreePath = useLoops((s) => (sel ? s.runs[sel]?.worktreePath : undefined));
  const runPaneId = useLoops((s) => (sel ? s.runs[sel]?.paneId : undefined));
  const runWsId = useLoops((s) => (sel ? s.runs[sel]?.wsId : undefined));

  // jump to the pane the run is happening in: open its space, make it the visible tab, focus it
  const watchRun = () => {
    if (!runWsId || !runPaneId) return;
    const W = useWorkspaces.getState();
    W.setActive(runWsId);
    const ws = W.workspaces.find((w) => w.id === runWsId);
    const pane = ws?.sessions.find((x) => x.id === runPaneId);
    if (ws && pane?.group) W.setActiveTab(runWsId, pane.group, runPaneId);
    W.setFocused(runPaneId);
    useUi.getState().goSpace();
  };
  const status = (sel && statuses[sel]) || "idle";
  const active = status === "running" || status === "paused";
  // runs happen in a pane on the logged-in claude CLI — no API key involved
  const canStart = !!def && !!def.prompt.trim() && !!def.folder;
  const startHint = !def
    ? ""
    : !def.prompt.trim() || !def.folder
      ? "Set a prompt and folder in the editor first"
      : "Run";

  return (
    <div className="loops-page">
      <div className="loops-page-inner wide">
        <div className="loops-page-head">
          <Repeat size={20} strokeWidth={1.75} />
          <div>
            <h1>Automations</h1>
            <p>Agents that run on a schedule, on an interval, or until the job's done.</p>
          </div>
        </div>

        {ids.length === 0 ? (
          <div className="loops-empty-runs">
            <p>No automations yet — create one to start automating.</p>
            <button className="btn" onClick={addLoop}>
              <Plus size={14} /> Create an automation
            </button>
          </div>
        ) : (
          <div className="loops-split">
            <div className="loops-master" ref={listRef}>
              {list.map((d) => {
                const st = statuses[d.id] ?? "idle";
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
                          <button
                            className="svc-run"
                            title={status === "paused" ? "Resume" : "Pause"}
                            onClick={() => pauseLoop(sel, status !== "paused")}
                          >
                            <Pause size={13} />
                          </button>
                          <button className="svc-stop" title="Stop" onClick={() => stopLoop(sel)}>
                            <Square size={13} />
                          </button>
                        </>
                      ) : (
                        <button className="svc-run" title={startHint} disabled={!canStart} onClick={() => startLoop(sel)}>
                          <Play size={13} />
                        </button>
                      )}
                      {runPaneId && runWsId && (
                        <button className="loop-review" title="Open the pane this run is using" onClick={watchRun}>
                          <Eye size={12} /> Watch
                        </button>
                      )}
                      {worktreePath && (
                        <button
                          className="loop-review"
                          title={`Open the isolated worktree:\n${worktreePath}`}
                          onClick={() => revealLoopWorktree(sel)}
                        >
                          <FolderGit2 size={12} /> Review changes
                        </button>
                      )}
                      <button
                        className="svc-run"
                        title={editing ? "Close editor" : "Edit"}
                        onClick={() => setEditing((v) => !v)}
                      >
                        <Settings2 size={13} />
                      </button>
                    </div>
                  </div>
                  {/* the run lives in a real pane now — this is its log, and the pane itself is in
                      the sidebar under the project it ran in */}
                  {editing ? (
                    <AutomationEditor id={sel} onClose={() => setEditing(false)} />
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
          const st = statuses[loopMenu.id];
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
                    setEditing(true);
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
