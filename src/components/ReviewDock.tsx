import { memo, useEffect, useRef, useState } from "react";
import { useUi } from "../stores/ui";
import { useWorkspaces } from "../stores/workspace";
import { gitChanges, gitDiff, gitBranchInfo, gitFileOp, gitCommit, type BranchInfo } from "../api";
import type { FileChange } from "../api/types";
import { SkillsPanel } from "./SkillsPanel";
import { FilesPanel } from "./FilesPanel";
import { confirmDialog } from "../stores/confirm";
import { kbd } from "../platform";
import { useNotifications } from "../stores/notifications";
import { ChevronRight, GitBranch, Zap, Plus, Minus, Undo2, FolderTree } from "lucide-react";

// porcelain code → a coarse class for the status chip color
function statusClass(code: string): string {
  if (code.includes("?")) return "new";
  if (code.includes("A")) return "new";
  if (code.includes("D")) return "del";
  if (code.includes("R")) return "mod";
  return "mod";
}

const DIFF_LINE_CAP = 2000; // keep a giant lockfile diff from creating 50k dom nodes

const DiffView = memo(function DiffView({ text }: { text: string }) {
  // remember which diff was expanded, so switching files re-collapses without a flash
  const [expandedFor, setExpandedFor] = useState<string | null>(null);
  if (!text.trim()) return <div className="dock-empty">No diff to show.</div>;
  const lines = text.split("\n");
  const showAll = expandedFor === text;
  const shown = showAll ? lines : lines.slice(0, DIFF_LINE_CAP);
  const hidden = lines.length - shown.length;
  return (
    <pre className="diff">
      {shown.map((l, i) => {
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
      {hidden > 0 && (
        <button className="sc-mini" onClick={() => setExpandedFor(text)}>
          Show {hidden.toLocaleString()} more lines
        </button>
      )}
    </pre>
  );
});

// Full source-control panel: branch + ahead/behind, staged/unstaged with per-file
// stage/unstage/discard, a commit box (commits exactly what's staged), and a diff view.
function SourceControl({ cwd }: { cwd: string }) {
  const view = useUi((s) => s.view);
  const [files, setFiles] = useState<FileChange[]>([]);
  const [branch, setBranch] = useState<BranchInfo | null>(null);
  const [sel, setSel] = useState<string | null>(null);
  const [diff, setDiff] = useState("");
  const [msg, setMsg] = useState("");
  const [busy, setBusy] = useState(false);
  const inflight = useRef(false);

  const refresh = () => {
    if (!cwd) {
      setFiles([]);
      setBranch(null);
      return;
    }
    if (inflight.current) return; // a slow repo shouldn't stack up ticks
    inflight.current = true;
    Promise.allSettled([
      // only take fresh identities when something actually changed, so the panel
      // doesn't re-render every poll
      gitChanges(cwd).then((next) =>
        setFiles((prev) => (JSON.stringify(prev) === JSON.stringify(next) ? prev : next)),
      ),
      gitBranchInfo(cwd).then((next) =>
        setBranch((prev) => (JSON.stringify(prev) === JSON.stringify(next) ? prev : next)),
      ),
    ]).then(() => {
      inflight.current = false;
    });
  };
  useEffect(() => {
    const tick = () => {
      // don't poll while the window is hidden or we're off the workspace view
      if (document.hidden || view !== "space") return;
      refresh();
    };
    tick();
    const id = setInterval(tick, 4000); // keep fresh while the dock is open
    const onVis = () => {
      if (!document.hidden) tick(); // catch up right away on return
    };
    document.addEventListener("visibilitychange", onVis);
    return () => {
      clearInterval(id);
      document.removeEventListener("visibilitychange", onVis);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [cwd, view]);

  const staged = files.filter((f) => f.status[0] !== " " && f.status[0] !== "?");
  const unstaged = files.filter((f) => f.status[1] !== " ");

  const openDiff = (p: string) => {
    setSel(p);
    setDiff("");
    gitDiff(cwd, p)
      .then(setDiff)
      .catch((e) => setDiff(String(e)));
  };
  const op = async (operation: "stage" | "unstage" | "discard", path: string) => {
    setBusy(true);
    try {
      await gitFileOp(cwd, operation, path);
      refresh();
    } catch (e) {
      useNotifications.getState().add({ title: "Git", body: String(e) });
    } finally {
      setBusy(false);
    }
  };
  const discard = async (path: string) => {
    const ok = await confirmDialog({
      title: "Discard changes",
      message: `Discard all changes to "${path}"? This can't be undone.`,
      confirmLabel: "Discard",
      cancelLabel: "Cancel",
      danger: true,
    });
    if (ok) void op("discard", path);
  };
  const commit = async (push: boolean) => {
    if (!msg.trim() || busy) return;
    setBusy(true);
    try {
      const res = await gitCommit(cwd, msg.trim(), push, false);
      useNotifications.getState().add({ title: push ? "Committed & pushed" : "Committed", body: res });
      setMsg("");
      refresh();
    } catch (e) {
      useNotifications.getState().add({ title: "Commit failed", body: String(e) });
    } finally {
      setBusy(false);
    }
  };

  if (!cwd) return <div className="dock-empty">Open a project workspace to manage its git.</div>;
  if (branch && !branch.is_repo) return <div className="dock-empty">Not a git repository.</div>;

  const row = (f: FileChange, area: "staged" | "unstaged") => {
    const ch = area === "staged" ? f.status[0] : f.status[1];
    return (
      <div className={`sc-file${sel === f.path ? " active" : ""}`} key={area + "-" + f.path}>
        <button className="sc-file-main" onClick={() => openDiff(f.path)} title={f.path}>
          <span className={`change-status s-${statusClass(f.status)}`}>{ch === "?" ? "U" : ch}</span>
          <span className="change-path">{f.path}</span>
        </button>
        <span className="sc-file-acts">
          {area === "unstaged" && (
            <button className="sc-act" title="Discard changes" onClick={() => void discard(f.path)}>
              <Undo2 size={13} />
            </button>
          )}
          <button
            className="sc-act"
            title={area === "staged" ? "Unstage" : "Stage"}
            onClick={() => void op(area === "staged" ? "unstage" : "stage", f.path)}
          >
            {area === "staged" ? <Minus size={13} /> : <Plus size={13} />}
          </button>
        </span>
      </div>
    );
  };

  return (
    <div className="dock-body sc">
      <div className="sc-head">
        <GitBranch size={13} />
        <span className="sc-branch">{branch?.branch || "—"}</span>
        {branch?.upstream && (branch.ahead > 0 || branch.behind > 0) && (
          <span className="sc-track">
            {branch.ahead > 0 && <span>↑{branch.ahead}</span>}
            {branch.behind > 0 && <span>↓{branch.behind}</span>}
          </span>
        )}
      </div>

      <div className="sc-commit">
        <textarea
          className="sc-msg"
          placeholder={staged.length ? "Message (commits staged changes)" : "Stage files, then write a message"}
          value={msg}
          onChange={(e) => setMsg(e.target.value)}
          onKeyDown={(e) => {
            if ((e.metaKey || e.ctrlKey) && e.key === "Enter") void commit(false);
          }}
        />
        <div className="sc-commit-btns">
          <button className="btn" disabled={busy || !msg.trim() || !staged.length} onClick={() => void commit(false)}>
            Commit
          </button>
          <button
            className="btn primary"
            disabled={busy || !msg.trim() || !staged.length}
            onClick={() => void commit(true)}
          >
            Commit &amp; Push
          </button>
        </div>
      </div>

      {staged.length > 0 && (
        <div className="sc-sec">
          <div className="sc-sec-head">
            <span>Staged · {staged.length}</span>
            <button className="sc-mini" onClick={() => void op("unstage", "")}>
              Unstage all
            </button>
          </div>
          {staged.map((f) => row(f, "staged"))}
        </div>
      )}

      <div className="sc-sec">
        <div className="sc-sec-head">
          <span>Changes · {unstaged.length}</span>
          {unstaged.length > 0 && (
            <button className="sc-mini" onClick={() => void op("stage", "")}>
              Stage all
            </button>
          )}
        </div>
        {unstaged.length === 0 && staged.length === 0 && (
          <div className="dock-empty">Working tree clean.</div>
        )}
        {unstaged.map((f) => row(f, "unstaged"))}
      </div>

      {sel && <DiffView text={diff} />}
    </div>
  );
}

// Collapsible right "cockpit" dock — Source Control (git) + Skills. When closed it renders
// nothing, so the terminal grid gets the full width.
export function ReviewDock() {
  const open = useUi((s) => s.dockOpen);
  const tab = useUi((s) => s.dockTab);
  const view = useUi((s) => s.view);
  const ws = useWorkspaces((s) => s.workspaces.find((w) => w.id === s.activeId) ?? null);
  const focusedId = useWorkspaces((s) => s.focusedSessionId);
  // follow the focused pane's folder, so a worktree agent shows ITS git, not the repo root's
  const focused = ws?.sessions.find((s) => s.id === focusedId);
  const cwd = focused?.cwd ?? ws?.cwd ?? "";

  const [render, setRender] = useState(open);
  const [closing, setClosing] = useState(false);
  useEffect(() => {
    if (open) {
      setRender(true);
      setClosing(false);
    } else {
      setClosing(true);
    }
  }, [open]);

  if (!render || view === "home") return null;

  return (
    <div
      className={`dock${closing ? " closing" : ""}`}
      onAnimationEnd={() => {
        if (closing) setRender(false);
      }}
    >
      <div className="dock-tabs">
        <button
          className={`dock-tab${tab === "changes" ? " active" : ""}`}
          onClick={() => useUi.getState().setDockTab("changes")}
        >
          <GitBranch size={13} />
          Source
        </button>
        <button
          className={`dock-tab${tab === "files" ? " active" : ""}`}
          onClick={() => useUi.getState().setDockTab("files")}
        >
          <FolderTree size={13} />
          Files
        </button>
        <button
          className={`dock-tab${tab === "skills" ? " active" : ""}`}
          onClick={() => useUi.getState().setDockTab("skills")}
        >
          <Zap size={13} />
          Skills
        </button>
        <button className="dock-x" title={`Hide dock (${kbd("Ctrl+Shift+G")})`} onClick={() => useUi.getState().setDock(false)}>
          <ChevronRight size={16} />
        </button>
      </div>
      {tab === "changes" ? (
        <SourceControl cwd={cwd} />
      ) : tab === "files" ? (
        <FilesPanel />
      ) : (
        <SkillsPanel cwd={cwd} />
      )}
    </div>
  );
}
