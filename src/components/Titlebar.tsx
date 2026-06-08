import { getCurrentWindow } from "@tauri-apps/api/window";
import { useWorkspaces } from "../stores/workspace";
import { useUi } from "../stores/ui";

const win = getCurrentWindow();

export function Titlebar() {
  const workspaces = useWorkspaces((s) => s.workspaces);
  const activeId = useWorkspaces((s) => s.activeId);
  const setActive = useWorkspaces((s) => s.setActive);
  const removeWorkspace = useWorkspaces((s) => s.removeWorkspace);
  const addOpenSpace = useWorkspaces((s) => s.addOpenSpace);
  const toggleSettings = useUi((s) => s.toggleSettings);

  return (
    <div className="titlebar" data-tauri-drag-region>
      <div className="tb-left" data-tauri-drag-region>
        <span className="tb-logo">◆</span>
        <span className="tb-brand">HyprSpace</span>
      </div>

      <div className="tb-tabs" data-tauri-drag-region>
        {workspaces.map((w) => (
          <div
            key={w.id}
            className={`tb-tab ${w.id === activeId ? "active" : ""}`}
            onClick={() => setActive(w.id)}
            title={w.cwd || w.name}
          >
            <span className="dot" style={{ background: w.color }} />
            <span className="tb-tab-name">{w.name}</span>
            {w.sessions.length > 0 && <span className="tb-tab-count">{w.sessions.length}</span>}
            <button
              className="tb-tab-close"
              title="Clear space"
              onClick={(e) => {
                e.stopPropagation();
                removeWorkspace(w.id);
              }}
            >
              ×
            </button>
          </div>
        ))}
        <button className="tb-newtab" title="New open space" onClick={() => addOpenSpace()}>
          +
        </button>
      </div>

      <div className="tb-controls">
        <button className="tb-ctl" title="Settings" onClick={toggleSettings}>
          <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3">
            <line x1="2" y1="4.5" x2="14" y2="4.5" />
            <line x1="2" y1="11.5" x2="14" y2="11.5" />
            <circle cx="6" cy="4.5" r="1.7" fill="var(--surface-1)" />
            <circle cx="10" cy="11.5" r="1.7" fill="var(--surface-1)" />
          </svg>
        </button>
        <button className="tb-ctl" title="Minimize" onClick={() => win.minimize()}>
          <svg width="10" height="10" viewBox="0 0 10 10">
            <rect x="1" y="4.5" width="8" height="1" fill="currentColor" />
          </svg>
        </button>
        <button className="tb-ctl" title="Maximize" onClick={() => win.toggleMaximize()}>
          <svg width="10" height="10" viewBox="0 0 10 10">
            <rect x="1.5" y="1.5" width="7" height="7" fill="none" stroke="currentColor" strokeWidth="1" />
          </svg>
        </button>
        <button className="tb-ctl close" title="Close" onClick={() => win.close()}>
          <svg width="10" height="10" viewBox="0 0 10 10">
            <path d="M1 1 L9 9 M9 1 L1 9" stroke="currentColor" strokeWidth="1.2" />
          </svg>
        </button>
      </div>
    </div>
  );
}
