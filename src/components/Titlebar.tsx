import type { MouseEvent as ReactMouseEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { PanelRight, Plus } from "lucide-react";
import { useUi } from "../stores/ui";
import { isMac } from "../platform";
import { newSession } from "../actions";
import { Logo } from "./Logo";
import { UsageMeter } from "./UsageMeter";
import { LayoutPicker } from "./LayoutPicker";

const win = getCurrentWindow();

// The left rail is solid while the sidebar is out and hollow once it is tucked away.
function SidebarIcon({ open }: { open: boolean }) {
  return (
    <svg width={16} height={16} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.75} strokeLinecap="round" strokeLinejoin="round">
      <rect x="3" y="3" width="18" height="18" rx="2" />
      {open ? <path d="M5 3h4v18H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z" fill="currentColor" stroke="none" /> : <path d="M9 3v18" />}
    </svg>
  );
}

export function Titlebar() {
  const view = useUi((s) => s.view);
  const railHidden = useUi((s) => s.railCollapsed);
  const toggleRail = useUi((s) => s.toggleRail);
  const settingsOpen = useUi((s) => s.settingsOpen);
  const dockOpen = useUi((s) => s.dockOpen);

  // macOS will not move the window via data-tauri-drag-region with the overlay title bar style
  // (tauri #9503), so the drag is driven here. Only a direct hit on the marked element drags,
  // so popovers hanging off the bar never move the window. The top-left corner belongs to the
  // native traffic lights.
  const onChrome = (e: ReactMouseEvent) => (e.target as HTMLElement).hasAttribute("data-tauri-drag-region");
  const inTrafficLights = (e: ReactMouseEvent) => e.clientX < 80 && e.clientY < 36;
  const onTbDown = (e: ReactMouseEvent) => {
    if (inTrafficLights(e)) return;
    if (e.button === 0 && onChrome(e)) void win.startDragging().catch(() => {});
  };
  const onTbDblClick = (e: ReactMouseEvent) => {
    if (inTrafficLights(e)) return;
    if (onChrome(e)) void win.toggleMaximize().catch(() => {});
  };

  return (
    <div
      className={`titlebar${isMac ? " mac" : ""}`}
      data-tauri-drag-region
      onMouseDown={isMac ? onTbDown : undefined}
      onDoubleClick={isMac ? onTbDblClick : undefined}
    >
      <div className="tb-left" data-tauri-drag-region>
        <button className="tb-home" title="Home" onClick={() => useUi.getState().goHome()}>
          <span className="tb-logo">
            <Logo size={16} />
          </span>
        </button>
        <button className="tb-sidebar" title={railHidden ? "Show sidebar" : "Hide sidebar"} onClick={toggleRail}>
          <SidebarIcon open={!railHidden} />
        </button>
      </div>

      <div className="tb-controls" data-tauri-drag-region>
        {!settingsOpen && (
          <div className="tb-actions">
            <button className="tb-action" title="New thread (Ctrl+Shift+N)" onClick={newSession}>
              <Plus size={16} />
            </button>
            <LayoutPicker />
          </div>
        )}
        <UsageMeter />
        {view === "space" && (
          <button
            className={`tb-ctl${dockOpen ? " on" : ""}`}
            title="Files and git (Ctrl+Shift+G)"
            onClick={() => useUi.getState().toggleDock()}
          >
            <PanelRight size={14} strokeWidth={1.75} />
          </button>
        )}
        {/* Windows and Linux draw their own window controls; macOS keeps the native ones. */}
        {!isMac && (
          <>
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
          </>
        )}
      </div>
    </div>
  );
}
