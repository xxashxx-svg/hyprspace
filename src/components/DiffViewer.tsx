import { useEffect, useState } from "react";
import { FileCode2 } from "lucide-react";
import { gitDiff } from "../api";
import { useWorkspaces } from "../stores/workspace";
import { joinPath } from "../lib/projects";

const LINE_CAP = 2000;

interface Props {
  cwd: string;
  path: string;
  active: boolean;
}

function lineClass(l: string): string {
  if (/^(\+\+\+|---|diff |index |new file|deleted file|similarity|rename )/.test(l)) return "meta";
  if (l.startsWith("@@")) return "hunk";
  if (l.startsWith("+")) return "add";
  if (l.startsWith("-")) return "del";
  return "";
}

/** A file's working tree diff as a pane. Refreshes while on screen so edits show up as they land. */
export function DiffViewer({ cwd, path, active }: Props) {
  const [text, setText] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [all, setAll] = useState(false);

  useEffect(() => {
    if (!active) return;
    let alive = true;
    const load = () =>
      gitDiff(cwd, path)
        .then((t) => {
          if (!alive) return;
          setText((prev) => (prev === t ? prev : t));
          setErr(null);
        })
        .catch((e) => alive && setErr(String(e)));
    load();
    const id = setInterval(() => {
      if (!document.hidden) load();
    }, 5000);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, [cwd, path, active]);

  const lines = text ? text.split("\n") : [];
  const added = lines.filter((l) => l.startsWith("+") && !l.startsWith("+++")).length;
  const removed = lines.filter((l) => l.startsWith("-") && !l.startsWith("---")).length;
  const shown = all ? lines : lines.slice(0, LINE_CAP);
  const hidden = lines.length - shown.length;

  return (
    <div className="dv">
      <div className="dv-bar">
        <span className="dv-path" title={path}>
          {path}
        </span>
        {text !== null && (
          <span className="dv-counts">
            <span className="add">+{added}</span>
            <span className="del">−{removed}</span>
          </span>
        )}
        <button className="dv-btn" title="Open in the editor" onClick={() => useWorkspaces.getState().openPathTab(joinPath(cwd, path))}>
          <FileCode2 size={13} />
        </button>
      </div>
      {err ? (
        <div className="dv-empty">{err}</div>
      ) : text === null ? null : !text.trim() ? (
        <div className="dv-empty">No changes to show.</div>
      ) : (
        <pre className="dv-body">
          {shown.map((l, i) => (
            <div key={i} className={`dv-line ${lineClass(l)}`}>
              {l || " "}
            </div>
          ))}
          {hidden > 0 && (
            <button className="dv-more" onClick={() => setAll(true)}>
              Show {hidden.toLocaleString()} more lines
            </button>
          )}
        </pre>
      )}
    </div>
  );
}
