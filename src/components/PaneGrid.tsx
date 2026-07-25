import { lazy, Suspense, useCallback, useEffect, useRef, useState } from "react";
import type { MouseEvent as RMouseEvent, PointerEvent as RPointerEvent } from "react";
import { X, Plus, Maximize2, Minimize2, FolderSymlink, Terminal as TerminalIcon } from "lucide-react";
import { useWorkspaces, toSlots } from "../stores/workspace";
import type { Session } from "../stores/workspace";
import { useUi } from "../stores/ui";
import { TerminalPane, PROVIDER_ICONS } from "./TerminalPane";
import { ImageViewer } from "./ImageViewer";
const PaneEditor = lazy(() => import("./CodeEditor").then((m) => ({ default: m.CodeEditor })));
import { PaneAddMenu } from "./PaneAddMenu";
import { TabContextMenu } from "./TabContextMenu";
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
  const setActiveTab = useWorkspaces((s) => s.setActiveTab);
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
  // the grid tiles SLOTS, not sessions — a tabbed slot (a group) counts as one cell
  const activeCount = active ? toSlots(active.sessions).length : 0;
  const activeLayout = resolveLayout(activeCount, active?.layouts?.[activeCount]);
  const showGrid = !!active && active.sessions.length > 0;

  // provider picker opened from a slot's tab-strip + (mirrors the pane header's + menu)
  const [tabMenu, setTabMenu] = useState<
    { x: number; y: number; wsId: string; anchorId: string; anchorCommand?: string; cwd: string } | null
  >(null);
  // right-click a tab — the actions the old pane-header "…" menu used to hold
  const [tabCtx, setTabCtx] = useState<{ x: number; y: number; wsId: string; sessionId: string } | null>(
    null,
  );

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
  // tabbed slots aren't drag targets in v1 — their panes get inert grip handlers
  const noopGrip = useCallback(() => {}, []);
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
          const slots = toSlots(w.sessions);
          const layout = resolveLayout(slots.length, w.layouts?.[slots.length]);
          return slots.map((slot, si) => {
            const single = slot.sessions.length === 1; // drag/reorder is still per-pane
            const stored = slot.group ? w.activeTabByGroup?.[slot.group] : undefined;
            // fall back to the first pane if the stored active tab is gone
            const activeTab = stored && slot.sessions.some((ss) => ss.id === stored) ? stored : slot.sessions[0].id;
            // when maximized, only the slot holding the maximized pane stays on screen
            const slotHasMax = maxedHere && slot.sessions.some((ss) => ss.id === maximizedId);
            const cellVisible = isActiveWs && (!maxedHere || slotHasMax);
            const solo = slot.sessions[0];
            const place = layout.place(si);
            return (
              <div
                key={slot.group ?? solo.id}
                data-sid={single ? solo.id : undefined}
                className={`pane-cell tabbed${single && dragId === solo.id ? " dragging" : ""}${single && overId === solo.id ? " drop-over" : ""}`}
                style={{
                  display: cellVisible ? undefined : "none",
                  gridColumn: cellVisible && !maxedHere ? place.gridColumn : undefined,
                  gridRow: cellVisible && !maxedHere ? place.gridRow : undefined,
                }}
              >
                <div
                    className="pane-tabs"
                    onPointerDown={single ? (e) => {
                      // only the empty strip area drags — not a tab or a button
                      if ((e.target as HTMLElement).closest(".pane-tab, button")) return;
                      onGripDown(e, w.id, solo.id);
                    } : undefined}
                    onPointerMove={single ? onGripMove : undefined}
                    onPointerUp={single ? onGripUp : undefined}
                  >
                    {slot.sessions.map((ts) => {
                      const TIcon = PROVIDER_ICONS[ts.provider] ?? TerminalIcon;
                      return (
                        <div
                          key={ts.id}
                          className={`pane-tab${ts.id === activeTab ? " active" : ""}`}
                          // image tabs are named after the file, so two panes both show "1.png" —
                          // the full path in a tooltip is the only way to tell them apart
                          title={ts.image || ts.file || ts.title || ts.provider}
                          onContextMenu={(e: RMouseEvent) => {
                            e.preventDefault();
                            e.stopPropagation();
                            setTabCtx({ x: e.clientX, y: e.clientY, wsId: w.id, sessionId: ts.id });
                          }}
                          onMouseDown={() =>
                            slot.group ? setActiveTab(w.id, slot.group, ts.id) : onPaneFocus(ts.id)
                          }
                        >
                          <TIcon size={11} className="pane-tab-ico" />
                          <span className="pane-tab-title">{ts.title || ts.provider}</span>
                          <button
                            className="pane-tab-x"
                            title="Close tab"
                            onMouseDown={(e) => e.stopPropagation()}
                            onClick={(e) => {
                              e.stopPropagation();
                              onPaneClose(w.id, ts.id);
                            }}
                          >
                            <X size={13} strokeWidth={2.6} />
                          </button>
                        </div>
                      );
                    })}
                    {/* chrome-style: new-tab button sits right after the last tab, not off in the
                        controls group */}
                    <button
                      className="pane-tab-add"
                      title="Open a pane in this folder"
                      onClick={(e: RMouseEvent) => {
                        const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
                        const act = slot.sessions.find((ss) => ss.id === activeTab) ?? solo;
                        setTabMenu({
                          x: Math.min(r.left, window.innerWidth - 210),
                          y: r.bottom + 4,
                          wsId: w.id,
                          anchorId: act.id,
                          anchorCommand: act.command,
                          cwd: act.cwd ?? w.cwd,
                        });
                      }}
                    >
                      <Plus size={14} />
                    </button>
                    {/* identity + controls for the ACTIVE tab — deliberately the same folder label
                        and the same .pane-btn group a solo pane's header shows, so a slot looks the
                        same whether or not it happens to be tabbed */}
                    <span className="pane-tabs-gap" />
                  {(() => {
                      const act = slot.sessions.find((ss) => ss.id === activeTab) ?? solo;
                      const acwd = act.cwd ?? w.cwd ?? "";
                      let dir = acwd.split(/[\\/]/).filter(Boolean).pop();
                      // a pane auto-named after its folder ("lualink-rs" in a lualink-rs tab) would
                      // print the same word twice — only keep the folder when it says something new
                      if (dir && dir.toLowerCase() === (act.title ?? "").toLowerCase()) dir = undefined;
                      return (
                        <>
                          {dir && (
                            <span className="pane-cwd" title={acwd}>
                              <FolderSymlink size={11} className="pane-cwd-ico" />
                              {dir}
                            </span>
                          )}
                          <span className="pane-head-right">
                            <button
                              className="pane-btn"
                              title={maximizedId === act.id ? "Restore" : "Maximize"}
                              onClick={() => onPaneToggleMax(act.id)}
                            >
                              {maximizedId === act.id ? <Minimize2 size={12} /> : <Maximize2 size={12} />}
                            </button>
                            <button
                              className="pane-btn close"
                              title="Close pane"
                              onClick={() => onPaneClose(w.id, act.id)}
                            >
                              <X size={13} />
                            </button>
                          </span>
                        </>
                      );
                    })()}
                  </div>
                {slot.sessions.map((sess: Session) => {
                  // one pane is on screen per visible slot: the maximized pane when maximizing,
                  // else the slot's active tab. hidden tabs stay mounted (display:none) so their PTY lives.
                  const visible = cellVisible && (maxedHere ? sess.id === maximizedId : sess.id === activeTab);
                  // a "guest" pane sits in a project space but points at a different folder than the
                  // project (e.g. dragged in from an open space) — flag it so it's obvious at a glance
                  const guest = w.kind === "project" && (sess.cwd ?? w.cwd) !== w.cwd;
                  const pane = sess.image ? (
                    <ImageViewer path={sess.image} active={visible} onClose={() => onPaneClose(w.id, sess.id)} tabbed />
                  ) : sess.file ? (
                    <Suspense fallback={null}>
                      <PaneEditor path={sess.file} onClose={() => onPaneClose(w.id, sess.id)} tabbed />
                    </Suspense>
                  ) : (
                    <>
                      <TerminalPane
                        sessionId={sess.id}
                        wsId={w.id}
                        cwd={sess.cwd ?? w.cwd}
                        guest={guest}
                        command={sess.command}
                        provider={sess.provider}
                        title={sess.title}
                        started={sess.started}
                        active={visible}
                        focused={isActiveWs && focusedSessionId === sess.id && visible}
                        isMaxed={maximizedId === sess.id}
                        tabbed
                        onFocus={onPaneFocus}
                        onClose={onPaneClose}
                        onToggleMax={onPaneToggleMax}
                        onGripDown={noopGrip}
                        onGripMove={noopGrip}
                        onGripUp={noopGrip}
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
                    </>
                  );
                  // ALWAYS wrap each pane in the same element (a .pane-tab-body div), solo or tabbed —
                  // so a solo pane that becomes tabbed (opening an image tab) keeps the SAME wrapper and
                  // its TerminalPane/PTY is never unmounted (was a Fragment→div swap that killed claude).
                  // hidden tabs are display:none so their PTY lives.
                  return (
                    <div
                      key={sess.id}
                      className="pane-tab-body"
                      style={{ display: visible ? undefined : "none" }}
                      // an image pane has no TerminalPane to claim focus, so without this clicking one
                      // leaves focus on the last terminal and Ctrl+Shift+W closes the WRONG pane
                      onMouseDown={sess.image || sess.file ? () => onPaneFocus(sess.id) : undefined}
                    >
                      {pane}
                    </div>
                  );
                })}
              </div>
            );
          });
        })}
      </div>
      {tabCtx && <TabContextMenu ctx={tabCtx} onClose={() => setTabCtx(null)} />}
      {tabMenu && (
        <PaneAddMenu
          x={tabMenu.x}
          y={tabMenu.y}
          wsId={tabMenu.wsId}
          anchorId={tabMenu.anchorId}
          anchorCommand={tabMenu.anchorCommand}
          cwd={tabMenu.cwd}
          onClose={() => setTabMenu(null)}
        />
      )}
    </div>
  );
}
