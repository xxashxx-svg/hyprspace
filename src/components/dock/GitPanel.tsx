import { useEffect, useRef, useState } from "react";
import { ArrowUp, GitBranch, RefreshCw } from "lucide-react";
import { useUi } from "../../stores/ui";
import { useWorkspaces } from "../../stores/workspace";
import { useGit } from "../../stores/git";
import { confirmDialog } from "../../stores/confirm";
import { useNotifications } from "../../stores/notifications";
import { gitChanges, gitBranchInfo, gitFileOp, gitCommit, gitPush, type BranchInfo } from "../../api";
import type { FileChange } from "../../api/types";
import { joinPath } from "../../lib/projects";

function statusLetter(code: string): { cls: string; letter: string } {
  if (code.includes("?")) return { cls: "new", letter: "U" };
  if (code.includes("A")) return { cls: "new", letter: "A" };
  if (code.includes("D")) return { cls: "del", letter: "D" };
  if (code.includes("R")) return { cls: "mod", letter: "R" };
  return { cls: "mod", letter: "M" };
}

const isStaged = (f: FileChange) => f.status[0] !== " " && f.status[0] !== "?";

/**
 * Working tree and commit box for one folder, in the shape of GitHub Desktop: tick the files to
 * include, write a summary, commit to the current branch, push when ahead.
 */
export function GitPanel({ cwd }: { cwd: string }) {
  const view = useUi((s) => s.view);
  const [files, setFiles] = useState<FileChange[]>([]);
  const [branch, setBranch] = useState<BranchInfo | null>(null);
  const [summary, setSummary] = useState("");
  const [body, setBody] = useState("");
  const [busy, setBusy] = useState<"" | "commit" | "push" | "file">("");
  const [ctx, setCtx] = useState<{ x: number; y: number; file: FileChange } | null>(null);
  const inflight = useRef(false);

  const refresh = () => {
    if (!cwd || inflight.current) return;
    inflight.current = true;
    Promise.allSettled([
      // git lists staged files first; sorting by path keeps a row where it was when it is ticked
      gitChanges(cwd).then((raw) => {
        const next = [...raw].sort((a, b) => a.path.localeCompare(b.path));
        setFiles((prev) => (JSON.stringify(prev) === JSON.stringify(next) ? prev : next));
      }),
      gitBranchInfo(cwd).then((next) => setBranch((prev) => (JSON.stringify(prev) === JSON.stringify(next) ? prev : next))),
    ]).then(() => {
      inflight.current = false;
    });
  };
  useEffect(() => {
    setFiles([]);
    setBranch(null);
    const tick = () => {
      if (document.hidden || view !== "space") return;
      refresh();
    };
    tick();
    const id = setInterval(tick, 4000);
    const onVis = () => {
      if (!document.hidden) tick();
    };
    document.addEventListener("visibilitychange", onVis);
    return () => {
      clearInterval(id);
      document.removeEventListener("visibilitychange", onVis);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [cwd, view]);

  const staged = files.filter(isStaged);
  const allStaged = files.length > 0 && staged.length === files.length;

  // the diff opens as a pane in the grid, like a file from the Files tab
  const openDiff = (p: string) => useWorkspaces.getState().openDiffTab(cwd, p);
  const fileOp = async (op: "stage" | "unstage" | "discard", path: string) => {
    setBusy("file");
    try {
      await gitFileOp(cwd, op, path);
      refresh();
    } catch (e) {
      useNotifications.getState().add({ title: "Git", body: String(e) });
    } finally {
      setBusy("");
    }
  };
  const discard = async (f: FileChange) => {
    setCtx(null);
    const ok = await confirmDialog({
      title: "Discard changes",
      message: `Throw away every change to ${f.path}? There is no undo.`,
      confirmLabel: "Discard",
      cancelLabel: "Keep",
      danger: true,
    });
    if (ok) void fileOp("discard", f.path);
  };
  const commit = async (push: boolean) => {
    const msg = summary.trim() + (body.trim() ? "\n\n" + body.trim() : "");
    if (!summary.trim() || !staged.length || busy) return;
    setBusy("commit");
    try {
      await gitCommit(cwd, msg, push, false);
      setSummary("");
      setBody("");
      refresh();
    } catch (e) {
      useNotifications.getState().add({ title: "Commit failed", body: String(e) });
    } finally {
      setBusy("");
    }
  };
  const push = async () => {
    if (busy) return;
    setBusy("push");
    try {
      await gitPush(cwd);
      refresh();
    } catch (e) {
      useNotifications.getState().add({ title: "Push failed", body: String(e) });
    } finally {
      setBusy("");
    }
  };

  if (!cwd) return <div className="dock-empty">Open a folder to see its changes.</div>;
  if (branch && !branch.is_repo) {
    return (
      <div className="dock-empty">
        <p>This folder is not a git repository.</p>
        <button className="btn" onClick={() => useGit.getState().openInitRepo()}>
          Create a repository
        </button>
      </div>
    );
  }

  return (
    <div className="git">
      <div className="git-branch">
        <GitBranch size={13} />
        <span className="git-branch-name">{branch?.branch || (branch?.is_repo ? "No commits yet" : "…")}</span>
        {branch && branch.behind > 0 && (
          <span className="git-track" title={`${branch.behind} commits behind the remote`}>
            ↓{branch.behind}
          </span>
        )}
        {branch && branch.ahead > 0 ? (
          <button className="git-push" disabled={busy !== ""} onClick={() => void push()} title="Push to the remote">
            <ArrowUp size={12} />
            Push {branch.ahead}
          </button>
        ) : (
          <button className="git-refresh" title="Refresh" onClick={refresh}>
            <RefreshCw size={12} />
          </button>
        )}
      </div>

      <div className="git-files-head">
        <label className="git-check">
          <input
            type="checkbox"
            checked={allStaged}
            // a mixed selection shows as a dash
            ref={(el) => {
              if (el) el.indeterminate = !allStaged && staged.length > 0;
            }}
            disabled={!files.length || busy !== ""}
            onChange={() => void fileOp(allStaged ? "unstage" : "stage", "")}
          />
          <span>
            {files.length} changed {files.length === 1 ? "file" : "files"}
          </span>
        </label>
      </div>

      <div className="git-files">
        {files.length === 0 && <div className="git-clean">Nothing to commit. The working tree is clean.</div>}
        {files.map((f) => {
          const st = statusLetter(f.status);
          const slash = f.path.lastIndexOf("/");
          const dir = slash >= 0 ? f.path.slice(0, slash + 1) : "";
          const name = slash >= 0 ? f.path.slice(slash + 1) : f.path;
          return (
            <div
              key={f.path}
              className="git-file"
              onContextMenu={(e) => {
                e.preventDefault();
                setCtx({ x: e.clientX, y: e.clientY, file: f });
              }}
            >
              <input
                type="checkbox"
                checked={isStaged(f)}
                disabled={busy !== ""}
                onChange={() => void fileOp(isStaged(f) ? "unstage" : "stage", f.path)}
              />
              <button className="git-file-main" title={f.path} onClick={() => openDiff(f.path)}>
                <span className={`git-status ${st.cls}`}>{st.letter}</span>
                <span className="git-path">
                  {dir && <span className="git-dir">{dir}</span>}
                  <span className="git-name">{name}</span>
                </span>
              </button>
              <span className="git-delta">
                {f.added > 0 && <span className="add">+{f.added}</span>}
                {f.removed > 0 && <span className="del">−{f.removed}</span>}
              </span>
            </div>
          );
        })}
      </div>

      <div className="git-commit">
        <input
          className="git-summary"
          placeholder="Summary (required)"
          value={summary}
          onChange={(e) => setSummary(e.target.value)}
          onKeyDown={(e) => {
            if ((e.metaKey || e.ctrlKey) && e.key === "Enter") void commit(false);
          }}
        />
        <textarea
          className="git-body"
          placeholder="Description"
          value={body}
          rows={2}
          onChange={(e) => setBody(e.target.value)}
          onKeyDown={(e) => {
            if ((e.metaKey || e.ctrlKey) && e.key === "Enter") void commit(false);
          }}
        />
        <div className="git-commit-btns">
          <button className="btn primary" disabled={busy !== "" || !summary.trim() || !staged.length} onClick={() => void commit(false)}>
            {busy === "commit" ? "Committing…" : `Commit to ${branch?.branch || "branch"}`}
          </button>
          <button className="btn" disabled={busy !== "" || !summary.trim() || !staged.length} onClick={() => void commit(true)} title="Commit, then push">
            Commit and push
          </button>
        </div>
        <div className="git-commit-hint">
          {staged.length ? `${staged.length} of ${files.length} files ticked` : files.length ? "Tick the files to include" : ""}
        </div>
      </div>

      {ctx && (
        <>
          <div
            className="ctx-backdrop"
            onClick={() => setCtx(null)}
            onContextMenu={(e) => {
              e.preventDefault();
              setCtx(null);
            }}
          />
          <div className="ctx-menu" style={{ left: ctx.x, top: ctx.y }}>
            <button
              className="ctx-item"
              onClick={() => {
                useWorkspaces.getState().openPathTab(joinPath(cwd, ctx.file.path));
                setCtx(null);
              }}
            >
              <span>Open in editor</span>
            </button>
            <button
              className="ctx-item"
              onClick={() => {
                openDiff(ctx.file.path);
                setCtx(null);
              }}
            >
              <span>Show diff</span>
            </button>
            <div className="ctx-sep" />
            <button className="ctx-item danger" onClick={() => void discard(ctx.file)}>
              <span>Discard changes</span>
            </button>
          </div>
        </>
      )}
    </div>
  );
}
