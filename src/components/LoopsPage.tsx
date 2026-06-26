import { useEffect, useState } from "react";
import { useAutoAnimate } from "@formkit/auto-animate/react";
import { Repeat, Plus, Play, Square, Pause, Settings2, FolderGit2, Loader2 } from "lucide-react";
import { useLoops, newLoop } from "../stores/loops";
import { useUi } from "../stores/ui";
import { useWorkspaces } from "../stores/workspace";
import { startLoop, stopLoop, pauseLoop, revealLoopWorktree } from "../lib/loops";
import { secretHas } from "../api";
import { LoopRunView } from "./LoopRunView";
import { LoopsManager } from "./LoopsManager";

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
    def.name = "New loop";
    useLoops.getState().upsert(def);
    useUi.getState().focusLoop(def.id);
    useUi.getState().setLoopsTab("manage"); // drop into config to fill it in
  };

  const def = sel ? loops[sel] : null;
  const run = sel ? runs[sel] : undefined;
  const status = run?.status ?? "idle";
  const active = status === "running" || status === "paused";
  const canStart = !!def && !!def.prompt.trim() && !!def.folder && !(def.provider === "claude" && !hasKey);
  const startHint = !def
    ? ""
    : def.provider === "claude" && !hasKey
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
            <h1>Loops &amp; Automations</h1>
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
            <p>No loops yet — create one to start automating.</p>
            <button className="btn" onClick={() => useUi.getState().setLoopsTab("manage")}>
              <Plus size={14} /> Create a loop
            </button>
          </div>
        ) : (
          <div className="loops-split">
            <div className="loops-master" ref={listRef}>
              {list.map((d) => {
                const r = runs[d.id];
                const st = r?.status ?? "idle";
                return (
                  <button
                    key={d.id}
                    className={`loops-master-item${sel === d.id ? " active" : ""}`}
                    title={d.name || "Untitled loop"}
                    onClick={() => useUi.getState().focusLoop(d.id)}
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
                <Plus size={14} /> New loop
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
                  <LoopRunView id={sel} />
                </>
              ) : (
                <div className="loop-run-empty">Select a loop to see its activity.</div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
