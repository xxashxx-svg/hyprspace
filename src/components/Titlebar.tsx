import { getCurrentWindow } from "@tauri-apps/api/window";
import { useUi } from "../stores/ui";
import { Logo } from "./Logo";
import { NotificationPanel } from "./NotificationPanel";

const win = getCurrentWindow();

export function Titlebar() {
  const toggleSettings = useUi((s) => s.toggleSettings);

  return (
    <div className="titlebar" data-tauri-drag-region>
      <div className="tb-left" data-tauri-drag-region>
        <span className="tb-logo">
          <Logo size={16} />
        </span>
        <span className="tb-brand">HyprSpace</span>
      </div>

      {/* command center — workspaces live in the sidebar now, so this is the useful centerpiece */}
      <div className="tb-center" data-tauri-drag-region>
        <button
          className="tb-cmd"
          title="Search or run a command (Ctrl+K)"
          onClick={() => useUi.getState().setPalette(true)}
        >
          <svg width="13" height="13" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4">
            <circle cx="7" cy="7" r="4.5" />
            <line x1="10.5" y1="10.5" x2="14.5" y2="14.5" />
          </svg>
          <span className="tb-cmd-text">Search or run a command</span>
          <span className="tb-cmd-kbd">Ctrl K</span>
        </button>
      </div>

      <div className="tb-controls">
        <NotificationPanel />
        <button
          className="tb-ctl"
          title="Review dock — changes & run (Ctrl+Shift+G)"
          onClick={() => useUi.getState().toggleDock()}
        >
          <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3">
            <rect x="2" y="2.5" width="12" height="11" rx="1.5" />
            <line x1="10.5" y1="2.5" x2="10.5" y2="13.5" />
          </svg>
        </button>
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
