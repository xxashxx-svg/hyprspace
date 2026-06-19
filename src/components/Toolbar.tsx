import { useWorkspaces } from "../stores/workspace";
import { pickFolders } from "../api";
import { isWindows } from "../platform";

const CLAUDE_CMD = "claude --permission-mode auto";
const GEMINI_CMD = "gemini";
const WSL_CMD = "wsl";

export function Toolbar() {
  const ws = useWorkspaces((s) => s.workspaces.find((w) => w.id === s.activeId));
  const addSession = useWorkspaces((s) => s.addSession);
  const removeWorkspace = useWorkspaces((s) => s.removeWorkspace);

  if (!ws) return <div className="toolbar" />;
  const isOpen = ws.kind === "open";

  // projects launch in their own folder; open spaces ask which folder(s) per launch
  const launch = async (command?: string) => {
    if (isOpen) {
      const folders = await pickFolders();
      folders.forEach((f) => addSession(ws.id, command, f));
    } else {
      addSession(ws.id, command);
    }
  };

  return (
    <div className="toolbar">
      <div className="toolbar-left">
        <span className="dot" style={{ background: ws.color }} />
        <span className="toolbar-name">{ws.name}</span>
        {ws.cwd ? (
          <span className="toolbar-cwd">{ws.cwd}</span>
        ) : isOpen ? (
          <span className="toolbar-cwd">open space · launch in any folder</span>
        ) : null}
      </div>
      <div className="toolbar-actions">
        <button className="btn" onClick={() => launch()}>
          {isOpen ? "+ Terminal in folder" : "+ Terminal"}
        </button>
        {isWindows && (
          <button className="btn" onClick={() => launch(WSL_CMD)}>
            {isOpen ? "+ WSL in folder" : "+ WSL"}
          </button>
        )}
        <button className="btn secondary" onClick={() => launch(GEMINI_CMD)}>
          {isOpen ? "+ Gemini in folder" : "+ Gemini"}
        </button>
        <button className="btn primary" onClick={() => launch(CLAUDE_CMD)}>
          {isOpen ? "+ Claude in folder" : "+ Claude"}
        </button>
        <button className="btn ghost" title="Close space" onClick={() => removeWorkspace(ws.id)}>
          ✕
        </button>
      </div>
    </div>
  );
}
