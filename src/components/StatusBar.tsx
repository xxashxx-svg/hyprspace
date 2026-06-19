import { useEffect, useState } from "react";
import { useWorkspaces } from "../stores/workspace";
import { useUi } from "../stores/ui";
import { shellName } from "../api";
import { Folder, LayoutGrid } from "lucide-react";

export function StatusBar() {
  const ws = useWorkspaces((s) => s.workspaces.find((w) => w.id === s.activeId));
  const total = useWorkspaces((s) => s.workspaces.reduce((n, w) => n + w.sessions.length, 0));
  const view = useUi((s) => s.view);
  const [shell, setShell] = useState("");
  useEffect(() => {
    shellName()
      .then(setShell)
      .catch(() => {});
  }, []);

  // a contextual info strip — hidden on the Home dashboard. launch panes from the top New menu.
  if (view === "home") return null;

  return (
    <div className="statusbar">
      <div className="sb-left">
        <span className="sb-ico">
          {ws?.kind === "open" ? <LayoutGrid size={12} /> : <Folder size={12} />}
        </span>
        <span className="sb-name">{ws?.name ?? "—"}</span>
        <span className="sb-sep">·</span>
        <span className="sb-muted">{shell || "shell"}</span>
        {ws?.cwd && (
          <>
            <span className="sb-sep">·</span>
            <span className="sb-cwd">{ws.cwd}</span>
          </>
        )}
      </div>
      <div className="sb-right">
        <span className={`sb-dot${total > 0 ? " on" : ""}`} />
        <span className="sb-muted">
          {total} session{total === 1 ? "" : "s"} active
        </span>
      </div>
    </div>
  );
}
