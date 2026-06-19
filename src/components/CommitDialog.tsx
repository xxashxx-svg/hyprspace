import { useEffect, useState } from "react";
import { useGit, gitCwd } from "../stores/git";
import { gitChanges } from "../api";

// Modal for entering a commit message. Opened from the topbar "Commit & push" menu.
export function CommitDialog() {
  const open = useGit((s) => s.dialogOpen);
  const withPush = useGit((s) => s.withPush);
  const busy = useGit((s) => s.busy);
  const commit = useGit((s) => s.commit);
  const close = useGit((s) => s.close);

  const [msg, setMsg] = useState("");
  const [count, setCount] = useState<number | null>(null);
  const [folder, setFolder] = useState("");

  useEffect(() => {
    if (!open) {
      setMsg("");
      setCount(null);
      setFolder("");
      return;
    }
    const cwd = gitCwd();
    setFolder(cwd.split(/[\\/]/).filter(Boolean).pop() || "");
    gitChanges(cwd)
      .then((f) => setCount(f.length))
      .catch(() => setCount(null));
  }, [open]);

  if (!open) return null;

  const submit = () => {
    if (msg.trim() && !busy) void commit(msg.trim());
  };

  return (
    <div className="cd-overlay" onMouseDown={close}>
      <div className="cd" onMouseDown={(e) => e.stopPropagation()}>
        <div className="cd-head">
          <span className="cd-title">{withPush ? "Commit & push" : "Commit changes"}</span>
          <span className="cd-count">
            {[
              folder,
              count != null
                ? count === 0
                  ? "no changes"
                  : `${count} changed file${count === 1 ? "" : "s"}`
                : null,
            ]
              .filter(Boolean)
              .join(" · ")}
          </span>
        </div>
        <textarea
          className="cd-msg"
          placeholder="Commit message…"
          autoFocus
          value={msg}
          onChange={(e) => setMsg(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") close();
            if ((e.metaKey || e.ctrlKey) && e.key === "Enter") submit();
          }}
        />
        <div className="cd-foot">
          <span className="cd-hint">Stages all changes · ⌘/Ctrl + Enter</span>
          <div className="cd-actions">
            <button className="btn" onClick={close} disabled={busy}>
              Cancel
            </button>
            <button className="btn primary" onClick={submit} disabled={busy || !msg.trim()}>
              {busy ? "Working…" : withPush ? "Commit & push" : "Commit"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
