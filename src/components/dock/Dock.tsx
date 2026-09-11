import { useRef } from "react";
import type { PointerEvent as RPointerEvent } from "react";
import { FolderTree, GitBranch, X } from "lucide-react";
import { useUi } from "../../stores/ui";
import { useWorkspaces } from "../../stores/workspace";
import { useSettings } from "../../stores/settings";
import { FilesPanel } from "./FilesPanel";
import { GitPanel } from "./GitPanel";

/**
 * The right dock: the focused pane's folder as a file tree, and its git working tree. Hidden
 * unless toggled (titlebar button or Ctrl+Shift+G). The left edge drags to resize.
 */
export function Dock() {
  const open = useUi((s) => s.dockOpen);
  const tab = useUi((s) => s.dockTab);
  const view = useUi((s) => s.view);
  const width = useSettings((s) => s.dockWidth);
  const ws = useWorkspaces((s) => s.workspaces.find((w) => w.id === s.activeId) ?? null);
  const focusedId = useWorkspaces((s) => s.focusedSessionId);
  // follow the focused pane's folder, so a worktree agent shows its own git, not the repo root's
  const focused = ws?.sessions.find((s) => s.id === focusedId);
  const cwd = focused?.cwd || ws?.cwd || ws?.sessions.find((s) => s.cwd)?.cwd || "";

  const drag = useRef<{ x: number; w: number } | null>(null);
  const onDown = (e: RPointerEvent<HTMLDivElement>) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { x: e.clientX, w: width };
  };
  const onMove = (e: RPointerEvent<HTMLDivElement>) => {
    if (!drag.current) return;
    useSettings.getState().setDockWidth(drag.current.w + (drag.current.x - e.clientX));
  };
  const onUp = (e: RPointerEvent<HTMLDivElement>) => {
    drag.current = null;
    e.currentTarget.releasePointerCapture(e.pointerId);
  };

  if (!open || view !== "space") return null;
  return (
    <aside className="dock" style={{ width }}>
      <div className="dock-resize" onPointerDown={onDown} onPointerMove={onMove} onPointerUp={onUp} />
      <div className="dock-tabs">
        <button className={`dock-tab${tab === "files" ? " active" : ""}`} onClick={() => useUi.getState().setDockTab("files")}>
          <FolderTree size={13} />
          Files
        </button>
        <button className={`dock-tab${tab === "git" ? " active" : ""}`} onClick={() => useUi.getState().setDockTab("git")}>
          <GitBranch size={13} />
          Git
        </button>
        <button className="dock-x" title="Hide (Ctrl+Shift+G)" onClick={() => useUi.getState().setDock(false)}>
          <X size={14} />
        </button>
      </div>
      {tab === "files" ? <FilesPanel /> : <GitPanel cwd={cwd} />}
    </aside>
  );
}
