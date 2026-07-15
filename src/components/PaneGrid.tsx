import { useCallback, useEffect, useRef, useState } from "react";
import type { PointerEvent as RPointerEvent } from "react";
import { useWorkspaces } from "../stores/workspace";
import { useUi } from "../stores/ui";
import { TerminalPane } from "./TerminalPane";
import { Launchpad } from "./Launchpad";
import { Logo } from "./Logo";
import { closeSession } from "../actions";
import { resolveLayout } from "../lib/grid";

function cellSidAt(x: number, y: number): string | null {
  const el = document.elementFromPoint(x, y) as HTMLElement | null;
  return el?.closest<HTMLElement>(".pane-cell")?.dataset.sid ?? null;
}

// which space (a rail row) is under the cursor — for dragging a pane out into another space
function railWsAt(x: number, y: number): string | null {
  const el = document.elementFromPoint(x, y) as HTMLElement | null;
  return el?.closest<HTMLElement>(".rail-item-wrap")?.dataset.wsid ?? null;
}

export function PaneGrid() {
  const workspaces = useWorkspaces((s) => s.workspaces);
  const activeId = useWorkspaces((s) => s.activeId);
  const focusedSessionId = useWorkspaces((s) => s.focusedSessionId);
  const setFocused = useWorkspaces((s) => s.setFocused);
  const reorder = useWorkspaces((s) => s.reorderSessions);
  const moveToWs = useWorkspaces((s) => s.moveSessionToWorkspace);

  const maximizedId = useUi((s) => s.maximizedId);
  const toggleMaximized = useUi((s) => s.toggleMaximized);
  const fileDropId = useUi((s) => s.fileDropId);
  const skillDropId = useUi((s) => s.skillDropId);

  const drag = useRef<{ id: string; ws: string; sx: number; sy: number; active: boolean } | null>(
    null,
  );
  const [dragId, setDragId] = useState<string | null>(null);
  const [overId, setOverId] = useState<string | null>(null);
  // drag hit-testing rides rAF: pointermove can fire way faster than the frame rate, and each
  // pass costs two elementFromPoint calls — coalesce to one pass per frame on the latest coords
  const dragRaf = useRef(0);
  const dragPos = useRef({ x: 0, y: 0 });
  useEffect(() => () => {
    if (dragRaf.current) cancelAnimationFrame(dragRaf.current);
  }, []);

  const active = workspaces.find((w) => w.id === activeId) ?? null;
  const maxedHere = !!maximizedId && !!active && active.sessions.some((s) => s.id === maximizedId);
  const activeCount = active?.sessions.length ?? 0;
  const activeLayout = resolveLayout(activeCount, active?.layouts?.[activeCount]);
  const showGrid = !!active && active.sessions.length > 0;

  // Lazy pane mounting: only mount (and spawn a PTY/agent for) spaces you've actually opened this
  // run. Otherwise EVERY persisted session across EVERY space spawns a live process at launch — with
  // a dozen spaces that's many GB of Claude processes and a freeze. Once opened, a space stays
  // mounted so its state is kept and panes can move between opened spaces without a restart.
  const [activated, setActivated] = useState<Set<string>>(() => (activeId ? new Set([activeId]) : new Set()));
  useEffect(() => {
    if (activeId) setActivated((p) => (p.has(activeId) ? p : new Set(p).add(activeId)));
  }, [activeId]);

  // ONE stable reference per handler (store actions + setState setters are stable, drag is a ref),
  // so memoized TerminalPanes don't re-render when a sibling is focused or a drag updates overId.
  const onPaneFocus = useCallback((sid: string) => setFocused(sid), [setFocused]);
  const onPaneClose = useCallback((wsId: string, sid: string) => void closeSession(wsId, sid), []);
  const onPaneToggleMax = useCallback((sid: string) => toggleMaximized(sid), [toggleMaximized]);
  const onGripDown = useCallback(
    (e: RPointerEvent<HTMLDivElement>, wsId: string, sid: string) => {
      if (e.button !== 0) return;
      e.currentTarget.setPointerCapture?.(e.pointerId);
      drag.current = { id: sid, ws: wsId, sx: e.clientX, sy: e.clientY, active: false };
      setFocused(sid);
    },
    [setFocused],
  );
  const onGripMove = useCallback(
    (e: RPointerEvent<HTMLDivElement>) => {
      const d = drag.current;
      if (!d) return;
      if (!d.active) {
        if (Math.hypot(e.clientX - d.sx, e.clientY - d.sy) < 5) return;
        d.active = true;
        setDragId(d.id);
        useUi.getState().setPaneDrag(true);
      }
      dragPos.current = { x: e.clientX, y: e.clientY };
      if (dragRaf.current) return;
      dragRaf.current = requestAnimationFrame(() => {
        dragRaf.current = 0;
        const cur = drag.current;
        if (!cur?.active) return; // drag ended before the frame
        const { x, y } = dragPos.current;
        // hovering a different space in the rail → it becomes the drop target
        const overWs = railWsAt(x, y);
        if (overWs && overWs !== cur.ws) {
          setOverId(null);
          useUi.getState().setPaneDragOverWs(overWs);
        } else {
          useUi.getState().setPaneDragOverWs(null);
          const sid = cellSidAt(x, y);
          setOverId(sid && sid !== cur.id ? sid : null);
        }
      });
    },
    [setDragId, setOverId],
  );
  const onGripUp = useCallback(
    (e: RPointerEvent<HTMLDivElement>) => {
      const d = drag.current;
      drag.current = null;
      if (dragRaf.current) {
        cancelAnimationFrame(dragRaf.current);
        dragRaf.current = 0;
      }
      e.currentTarget.releasePointerCapture?.(e.pointerId);
      if (d?.active) {
        const overWs = railWsAt(e.clientX, e.clientY);
        if (overWs && overWs !== d.ws) {
          moveToWs(d.ws, d.id, overWs); // dropped onto another space → move it there
        } else {
          const target = cellSidAt(e.clientX, e.clientY);
          if (target && target !== d.id) reorder(d.ws, d.id, target);
        }
      }
      setDragId(null);
      setOverId(null);
      useUi.getState().setPaneDrag(false);
    },
    [moveToWs, reorder, setDragId, setOverId],
  );

  return (
    <div className={`pane-stage${dragId ? " dragging-active" : ""}`}>
      {!active && (
        <div className="pane-empty">
          <div className="empty-logo">
            <Logo size={28} />
          </div>
          <div className="empty-title">No spaces yet</div>
          <div className="empty-hint">Create a project or an open space from the sidebar to start</div>
        </div>
      )}
      {active && active.sessions.length === 0 && (
        <Launchpad wsId={active.id} name={active.name} kind={active.kind} cwd={active.cwd ?? ""} />
      )}

      {/* ONE grid holds every space's panes; inactive ones are display:none so their PTYs stay
          alive AND a pane can move between spaces without React remounting it — the key stays
          under the same parent, so the xterm + PTY survive the move instead of restarting. */}
      <div
        className={`pane-grid${maxedHere ? " maxed" : ""}`}
        style={{
          display: showGrid ? "grid" : "none",
          gridTemplateColumns: maxedHere ? "1fr" : activeLayout.cols,
          gridTemplateRows: maxedHere ? undefined : activeLayout.rows,
        }}
      >
        {workspaces.flatMap((w) => {
          if (!activated.has(w.id)) return []; // not opened yet this run — don't mount or spawn it
          const isActiveWs = w.id === activeId;
          const layout = resolveLayout(w.sessions.length, w.layouts?.[w.sessions.length]);
          return w.sessions.map((sess, i) => {
            const hiddenByMax = maxedHere && sess.id !== maximizedId;
            const visible = isActiveWs && !hiddenByMax;
            // a "guest" pane sits in a project space but points at a different folder than the
            // project (e.g. dragged in from an open space) — flag it so it's obvious at a glance
            const guest = w.kind === "project" && (sess.cwd ?? w.cwd) !== w.cwd;
            return (
              <div
                key={sess.id}
                data-sid={sess.id}
                className={`pane-cell${dragId === sess.id ? " dragging" : ""}${overId === sess.id ? " drop-over" : ""}`}
                style={{
                  display: visible ? undefined : "none",
                  gridColumn: visible && !maxedHere ? layout.place(i).gridColumn : undefined,
                  gridRow: visible && !maxedHere ? layout.place(i).gridRow : undefined,
                }}
              >
                <TerminalPane
                  sessionId={sess.id}
                  wsId={w.id}
                  cwd={sess.cwd ?? w.cwd}
                  guest={guest}
                  command={sess.command}
                  provider={sess.provider}
                  title={sess.title}
                  started={sess.started}
                  active={isActiveWs}
                  focused={isActiveWs && focusedSessionId === sess.id}
                  isMaxed={maximizedId === sess.id}
                  onFocus={onPaneFocus}
                  onClose={onPaneClose}
                  onToggleMax={onPaneToggleMax}
                  onGripDown={onGripDown}
                  onGripMove={onGripMove}
                  onGripUp={onGripUp}
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
                {skillDropId === sess.id && (
                  <div className="file-drop-overlay">
                    <div className="fdo-card">
                      <div className="fdo-icon">⌁</div>
                      <div className="fdo-title">Drop to insert</div>
                      <div className="fdo-sub">inserts this skill into the terminal</div>
                    </div>
                  </div>
                )}
              </div>
            );
          });
        })}
      </div>
    </div>
  );
}
