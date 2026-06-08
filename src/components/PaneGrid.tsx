import { useRef, useState } from "react";
import { useWorkspaces } from "../stores/workspace";
import { useUi } from "../stores/ui";
import { TerminalPane } from "./TerminalPane";
import { pickFolders } from "../api";

const CLAUDE_CMD = "claude --permission-mode auto";

function gridColumns(n: number): string {
  if (n <= 1) return "1fr";
  if (n === 2) return "1fr 1fr";
  if (n === 3) return "1fr 1fr 1fr";
  if (n === 4) return "1fr 1fr";
  return "1fr 1fr 1fr";
}

function cellSidAt(x: number, y: number): string | null {
  const el = document.elementFromPoint(x, y) as HTMLElement | null;
  return el?.closest<HTMLElement>(".pane-cell")?.dataset.sid ?? null;
}

export function PaneGrid() {
  const workspaces = useWorkspaces((s) => s.workspaces);
  const activeId = useWorkspaces((s) => s.activeId);
  const focusedSessionId = useWorkspaces((s) => s.focusedSessionId);
  const setFocused = useWorkspaces((s) => s.setFocused);
  const removeSession = useWorkspaces((s) => s.removeSession);
  const reorder = useWorkspaces((s) => s.reorderSessions);
  const addSession = useWorkspaces((s) => s.addSession);

  const maximizedId = useUi((s) => s.maximizedId);
  const toggleMaximized = useUi((s) => s.toggleMaximized);
  const fileDropId = useUi((s) => s.fileDropId);

  const drag = useRef<{ id: string; sx: number; sy: number; active: boolean } | null>(null);
  const [dragId, setDragId] = useState<string | null>(null);
  const [overId, setOverId] = useState<string | null>(null);

  const active = workspaces.find((w) => w.id === activeId) ?? null;

  const launch = async (command?: string) => {
    if (!active) return;
    if (active.kind === "open") {
      const folders = await pickFolders();
      folders.forEach((f) => addSession(active.id, command, f));
    } else {
      addSession(active.id, command);
    }
  };

  return (
    <div className={`pane-stage${dragId ? " dragging-active" : ""}`}>
      {!active && (
        <div className="pane-empty">
          <div className="empty-mark">›_</div>
          <div className="empty-title">No spaces yet</div>
          <div className="empty-hint">Create a project or an open space from the sidebar to start</div>
        </div>
      )}
      {active && active.sessions.length === 0 && (
        <div className="pane-empty">
          <div className="empty-mark">›_</div>
          <div className="empty-title">{active.name}</div>
          <div className="empty-hint">
            {active.kind === "open"
              ? "Launch Claude or a terminal in any folder — pick one or several"
              : "Launch a terminal or Claude to start working"}
          </div>
          <div className="empty-actions">
            <button className="btn primary" onClick={() => launch(CLAUDE_CMD)}>
              {active.kind === "open" ? "+ Claude in folder(s)" : "+ Claude"}
            </button>
            <button className="btn" onClick={() => launch()}>
              {active.kind === "open" ? "+ Terminal in folder(s)" : "+ Terminal"}
            </button>
          </div>
        </div>
      )}

      {/* render EVERY workspace's grid; hide inactive with display:none so PTYs stay alive */}
      {workspaces.map((w) => {
        if (w.sessions.length === 0) return null;
        const isActive = w.id === activeId;
        const maxedHere = isActive && !!maximizedId && w.sessions.some((s) => s.id === maximizedId);
        return (
          <div
            key={w.id}
            className="pane-grid"
            style={{
              gridTemplateColumns: maxedHere ? "1fr" : gridColumns(w.sessions.length),
              display: isActive ? "grid" : "none",
            }}
          >
            {w.sessions.map((sess) => {
              // keep all panes mounted (PTYs alive); hide the non-maxed ones when one is zoomed
              const hidden = maxedHere && sess.id !== maximizedId;
              return (
                <div
                  key={sess.id}
                  data-sid={sess.id}
                  className={`pane-cell${dragId === sess.id ? " dragging" : ""}${overId === sess.id ? " drop-over" : ""}`}
                  style={hidden ? { display: "none" } : undefined}
                >
                  <TerminalPane
                    sessionId={sess.id}
                    cwd={sess.cwd ?? w.cwd}
                    command={sess.command}
                    started={sess.started}
                    active={isActive}
                    focused={isActive && focusedSessionId === sess.id}
                    isMaxed={maximizedId === sess.id}
                    onFocus={() => setFocused(sess.id)}
                    onClose={() => removeSession(w.id, sess.id)}
                    onToggleMax={() => toggleMaximized(sess.id)}
                    onGripDown={(e) => {
                      if (e.button !== 0) return;
                      e.currentTarget.setPointerCapture?.(e.pointerId);
                      drag.current = { id: sess.id, sx: e.clientX, sy: e.clientY, active: false };
                      setFocused(sess.id);
                    }}
                    onGripMove={(e) => {
                      const d = drag.current;
                      if (!d) return;
                      if (!d.active) {
                        if (Math.hypot(e.clientX - d.sx, e.clientY - d.sy) < 5) return;
                        d.active = true;
                        setDragId(d.id);
                      }
                      const sid = cellSidAt(e.clientX, e.clientY);
                      setOverId(sid && sid !== d.id ? sid : null);
                    }}
                    onGripUp={(e) => {
                      const d = drag.current;
                      drag.current = null;
                      e.currentTarget.releasePointerCapture?.(e.pointerId);
                      if (d?.active) {
                        const target = cellSidAt(e.clientX, e.clientY);
                        if (target && target !== d.id) reorder(w.id, d.id, target);
                      }
                      setDragId(null);
                      setOverId(null);
                    }}
                  />
                  {fileDropId === sess.id && (
                    <div className="file-drop-overlay">
                      <div className="fdo-card">
                        <div className="fdo-icon">⤓</div>
                        <div className="fdo-title">Drop to insert</div>
                        <div className="fdo-sub">adds the path(s) to this terminal</div>
                      </div>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        );
      })}
    </div>
  );
}
