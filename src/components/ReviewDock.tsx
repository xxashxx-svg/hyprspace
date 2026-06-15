import { useEffect, useState } from "react";
import { useUi } from "../stores/ui";
import { useWorkspaces } from "../stores/workspace";
import { gitChanges, gitDiff } from "../api";
import type { FileChange } from "../api/types";
import { RunPanel } from "./RunPanel";

// porcelain code → a coarse class for the status chip color
function statusClass(code: string): string {
  if (code.includes("?")) return "new";
  if (code.includes("A")) return "new";
  if (code.includes("D")) return "del";
  if (code.includes("R")) return "mod";
  return "mod";
}

function DiffView({ text }: { text: string }) {
  if (!text.trim()) return <div className="dock-empty">No diff to show.</div>;
  return (
    <pre className="diff">
      {text.split("\n").map((l, i) => {
        let cls = "";
        if (l.startsWith("+++") || l.startsWith("---") || l.startsWith("diff ") || l.startsWith("index "))
          cls = "d-meta";
        else if (l.startsWith("@@")) cls = "d-hunk";
        else if (l.startsWith("+")) cls = "d-add";
        else if (l.startsWith("-")) cls = "d-del";
        return (
          <div key={i} className={`d-line ${cls}`}>
            {l || " "}
          </div>
        );
      })}
    </pre>
  );
}

function ChangesView({ cwd }: { cwd: string }) {
  const [files, setFiles] = useState<FileChange[]>([]);
  const [sel, setSel] = useState<string | null>(null);
  const [diff, setDiff] = useState("");

  useEffect(() => {
    let stop = false;
    const refresh = () => {
      if (!cwd) {
        setFiles([]);
        return;
      }
      gitChanges(cwd)
        .then((f) => {
          if (!stop) setFiles(f);
        })
        .catch(() => {});
    };
    refresh();
    const id = setInterval(refresh, 4000); // keep it fresh while the dock is open
    return () => {
      stop = true;
      clearInterval(id);
    };
  }, [cwd]);

  const openFile = (p: string) => {
    setSel(p);
    setDiff("");
    gitDiff(cwd, p)
      .then(setDiff)
      .catch((e) => setDiff(String(e)));
  };

  if (!cwd) return <div className="dock-empty">Open a project workspace to see its changes.</div>;

  return (
    <div className="dock-body">
      <div className="changes-list">
        {files.length === 0 && <div className="dock-empty">No changes — working tree clean.</div>}
        {files.map((f) => (
          <button
            key={f.path}
            className={`change-item${sel === f.path ? " active" : ""}`}
            onClick={() => openFile(f.path)}
            title={f.path}
          >
            <span className={`change-status s-${statusClass(f.status)}`}>{f.status || "·"}</span>
            <span className="change-path">{f.path}</span>
            {f.added + f.removed > 0 && (
              <span className="change-num">
                <span className="add">+{f.added}</span>
                <span className="del">−{f.removed}</span>
              </span>
            )}
          </button>
        ))}
      </div>
      {sel && <DiffView text={diff} />}
    </div>
  );
}

// Collapsible right "cockpit" dock — Changes (git diff) + Run (dev server). When closed it
// renders nothing, so the terminal grid gets the full width exactly like before.
export function ReviewDock() {
  const open = useUi((s) => s.dockOpen);
  const tab = useUi((s) => s.dockTab);
  const ws = useWorkspaces((s) => s.workspaces.find((w) => w.id === s.activeId) ?? null);
  const cwd = ws?.cwd ?? "";

  if (!open) return null;

  return (
    <div className="dock">
      <div className="dock-tabs">
        <button
          className={`dock-tab${tab === "changes" ? " active" : ""}`}
          onClick={() => useUi.getState().setDockTab("changes")}
        >
          Changes
        </button>
        <button
          className={`dock-tab${tab === "run" ? " active" : ""}`}
          onClick={() => useUi.getState().setDockTab("run")}
        >
          Run
        </button>
        <button className="dock-x" title="Hide dock (Ctrl+Shift+G)" onClick={() => useUi.getState().setDock(false)}>
          ›
        </button>
      </div>
      {tab === "changes" ? (
        <ChangesView cwd={cwd} />
      ) : (
        <RunPanel wsId={ws?.id ?? "none"} cwd={cwd} />
      )}
    </div>
  );
}
