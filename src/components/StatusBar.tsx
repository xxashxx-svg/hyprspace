import { useWorkspaces } from "../stores/workspace";
import { pickFolders } from "../api";

const CLAUDE_CMD = "claude --permission-mode auto";

export function StatusBar() {
  const ws = useWorkspaces((s) => s.workspaces.find((w) => w.id === s.activeId));
  const total = useWorkspaces((s) => s.workspaces.reduce((n, w) => n + w.sessions.length, 0));
  const addSession = useWorkspaces((s) => s.addSession);

  const launch = async (command?: string) => {
    if (!ws) return;
    if (ws.kind === "open") {
      const folders = await pickFolders();
      folders.forEach((f) => addSession(ws.id, command, f));
    } else {
      addSession(ws.id, command);
    }
  };

  return (
    <div className="statusbar">
      <div className="sb-left">
        {ws && <span className="dot" style={{ background: ws.color }} />}
        <span className="sb-name">{ws?.name ?? "—"}</span>
        <span className="sb-sep">·</span>
        <span className="sb-muted">powershell</span>
        {ws?.cwd && (
          <>
            <span className="sb-sep">·</span>
            <span className="sb-muted sb-cwd">{ws.cwd}</span>
          </>
        )}
      </div>
      <div className="sb-right">
        <span className="sb-muted">
          {total} session{total === 1 ? "" : "s"} active
        </span>
        <button className="sb-btn" onClick={() => launch()}>
          + terminal
        </button>
        <button className="sb-btn accent" onClick={() => launch(CLAUDE_CMD)}>
          + claude
        </button>
      </div>
    </div>
  );
}
