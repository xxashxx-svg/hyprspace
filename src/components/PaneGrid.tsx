import { lazy, Suspense, useCallback, useEffect, useRef, useState } from "react";
import type { MouseEvent as RMouseEvent, PointerEvent as RPointerEvent } from "react";
import { X, GripVertical, Terminal as TerminalIcon } from "lucide-react";
import { useWorkspaces, toSlots } from "../stores/workspace";
import type { Session } from "../stores/workspace";
import { useUi } from "../stores/ui";
import { TerminalPane, PROVIDER_ICONS } from "./TerminalPane";
import { ImageViewer } from "./ImageViewer";
import { MediaViewer } from "./MediaViewer";
import { DiffViewer } from "./DiffViewer";
const PaneEditor = lazy(() => import("./CodeEditor").then((m) => ({ default: m.CodeEditor })));
import { TabContextMenu } from "./TabContextMenu";
import { ComposerPane } from "./composer/ComposerPane";
import { Logo } from "./Logo";
import { closeSession } from "../actions";
import { PROVIDER_LOGO } from "../lib/brand";
import { resolveLayout, activeLayoutId, resizableBoundaries, trackCount, weightsTemplate } from "../lib/grid";

// the proxy is capped rather than full-size: a third-of-the-screen card in your hand would cover
// the very drop targets you're aiming at
const GHOST_MAX_W = 300;
const GHOST_MAX_H = 190;

function cellSidAt(x: number, y: number): string | null {
  const el = document.elementFromPoint(x, y) as HTMLElement | null;
  return el?.closest<HTMLElement>(".pane-cell")?.dataset.sid ?? null;
}

// which space's rows in the sidebar are under the cursor, for dragging a pane into another space
function railWsAt(x: number, y: number): string | null {
  const el = document.elementFromPoint(x, y) as HTMLElement | null;
  return el?.closest<HTMLElement>(".space-row, .sess-row")?.dataset.wsid ?? null;
}

export function PaneGrid() {
  const workspaces = useWorkspaces((s) => s.workspaces);
  const activeId = useWorkspaces((s) => s.activeId);
  const focusedSessionId = useWorkspaces((s) => s.focusedSessionId);
  const setFocused = useWorkspaces((s) => s.setFocused);
  const setActiveTab = useWorkspaces((s) => s.setActiveTab);
  const reorder = useWorkspaces((s) => s.reorderSessions);
  const moveToWs = useWorkspaces((s) => s.moveSessionToWorkspace);

  const maximizedByWs = useUi((s) => s.maximizedByWs);
  const toggleMaximized = useUi((s) => s.toggleMaximized);
  const fileDropId = useUi((s) => s.fileDropId);
  const skillDropId = useUi((s) => s.skillDropId);

  const drag = useRef<{
    id: string;
    ws: string;
    sx: number;
    sy: number;
    active: boolean;
    // where inside the pane you grabbed it, so the ghost stays under that same point instead of
    // snapping its corner to the cursor
    gx: number;
    gy: number;
  } | null>(null);
  const [dragId, setDragId] = useState<string | null>(null);
  const [overId, setOverId] = useState<string | null>(null);
  // The thing you actually "pick up". The live pane can't be lifted: giving .terminal-pane a
  // transform/shadow/z-index promotes it to its own compositing layer and WebView2 then
  // mis-composites xterm's WebGL canvas (see the note in pane.css). So a proxy flies instead and
  // the real pane never moves. Set once when the drag starts; the position is written straight to
  // the DOM in the rAF below, so a pointermove never re-renders the grid.
  const ghostRef = useRef<HTMLDivElement>(null);
  const [ghost, setGhost] = useState<{
    title: string;
    provider: Session["provider"];
    folder?: string;
    w: number;
    h: number;
  } | null>(null);
  // drag hit-testing rides rAF: pointermove can fire way faster than the frame rate, and each
  // pass costs two elementFromPoint calls — coalesce to one pass per frame on the latest coords
  const dragRaf = useRef(0);
  const dragPos = useRef({ x: 0, y: 0 });
  const dragRect = useRef<{ w: number; h: number } | null>(null);
  const ghostK = useRef(1);
  // Written straight to the node, never through state: at 60fps a setState per move would re-render
  // the whole grid (and every live terminal in it) while you drag.
  const moveGhost = useCallback((x: number, y: number, d: { gx: number; gy: number }) => {
    const g = ghostRef.current;
    if (!g) return;
    const k = ghostK.current;
    g.style.transform = `translate3d(${x - d.gx * k}px, ${y - d.gy * k}px, 0)`;
  }, []);
  useEffect(() => () => {
    if (dragRaf.current) cancelAnimationFrame(dragRaf.current);
  }, []);

  const active = workspaces.find((w) => w.id === activeId) ?? null;
  // the ACTIVE project's fullscreen pane; other projects keep their own, untouched
  const maximizedId = active ? maximizedByWs[active.id] : undefined;
  const maxedHere = !!maximizedId && !!active && active.sessions.some((s) => s.id === maximizedId);
  // the grid tiles SLOTS, not sessions — a tabbed slot (a group) counts as one cell
  const activeCount = active ? toSlots(active.sessions).length : 0;
  const activeLayout = resolveLayout(activeCount, active?.layouts?.[activeCount]);
  const showGrid = !!active && active.sessions.length > 0;

  // Dragged track weights for this layout, if any. Written straight to the grid element while
  // dragging and stored on release, so a drag never re-renders the terminals.
  const layoutKey = `${activeCount}:${activeLayoutId(activeCount, active?.layouts?.[activeCount])}`;
  const tracks = active?.tracks?.[layoutKey];
  const preset = activeLayout.preset;
  const colW = preset && tracks?.cols?.length === trackCount(preset.cols) ? tracks.cols : preset ? Array(trackCount(preset.cols)).fill(1) : null;
  const rowW = preset && tracks?.rows?.length === trackCount(preset.rows) ? tracks.rows : preset ? Array(trackCount(preset.rows)).fill(1) : null;
  const gridCols = colW ? weightsTemplate(colW) : activeLayout.cols;
  const gridRows = rowW ? weightsTemplate(rowW) : activeLayout.rows;
  const gridRef = useRef<HTMLDivElement>(null);
  const setTracks = useWorkspaces((s) => s.setTracks);
  // where each draggable boundary sits, as a css calc over the grid's padding and gaps
  const gutterPos = (w: number[], b: number) => {
    const total = w.reduce((a, x) => a + x, 0);
    const frac = w.slice(0, b).reduce((a, x) => a + x, 0) / total;
    return `calc(8px + (100% - 16px - ${8 * (w.length - 1)}px) * ${frac} + ${8 * (b - 1) + 4}px)`;
  };
  const gutter = useRef<{ axis: "col" | "row"; b: number; start: number; w: number[]; inner: number } | null>(null);
  const onGutterDown = (e: RPointerEvent<HTMLDivElement>, axis: "col" | "row", b: number) => {
    const g = gridRef.current;
    const w = axis === "col" ? colW : rowW;
    if (!g || !w) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    const r = g.getBoundingClientRect();
    const inner = (axis === "col" ? r.width : r.height) - 16 - 8 * (w.length - 1);
    gutter.current = { axis, b, start: axis === "col" ? e.clientX : e.clientY, w: [...w], inner };
  };
  const onGutterMove = (e: RPointerEvent<HTMLDivElement>) => {
    const d = gutter.current;
    const g = gridRef.current;
    if (!d || !g) return;
    const total = d.w.reduce((a, b) => a + b, 0);
    const delta = (((d.axis === "col" ? e.clientX : e.clientY) - d.start) / d.inner) * total;
    const min = total * 0.12;
    const next = [...d.w];
    const a = Math.max(min, Math.min(d.w[d.b - 1] + delta, d.w[d.b - 1] + d.w[d.b] - min));
    next[d.b - 1] = a;
    next[d.b] = d.w[d.b - 1] + d.w[d.b] - a;
    // The grid template is written straight to the DOM so a drag never re-renders the terminals —
    // which also means React never repositions the handle you are holding, and it sits at its old
    // offset until you let go. Move it along with the boundary it stands for. Only this one moves:
    // the two weights either side of it always sum to what they did, so the rest stay put.
    const el = e.currentTarget;
    if (d.axis === "col") {
      g.style.gridTemplateColumns = weightsTemplate(next);
      el.style.left = gutterPos(next, d.b);
    } else {
      g.style.gridTemplateRows = weightsTemplate(next);
      el.style.top = gutterPos(next, d.b);
    }
    (d as { live?: number[] }).live = next;
  };
  const onGutterUp = (e: RPointerEvent<HTMLDivElement>) => {
    const d = gutter.current as ({ live?: number[] } & NonNullable<typeof gutter.current>) | null;
    gutter.current = null;
    e.currentTarget.releasePointerCapture(e.pointerId);
    if (!d?.live || !active) return;
    setTracks(active.id, layoutKey, d.axis === "col" ? { cols: d.live } : { rows: d.live });
  };

  // right-click a tab — the actions the old pane-header "…" menu used to hold
  const [tabCtx, setTabCtx] = useState<{ x: number; y: number; wsId: string; sessionId: string } | null>(
    null,
  );

  // Lazy pane mounting: only mount (and spawn a PTY/agent for) spaces you've actually opened this
  // run. Otherwise EVERY persisted session across EVERY space spawns a live process at launch — with
  // a dozen spaces that's many GB of Claude processes and a freeze. Once opened, a space stays
  // mounted so its state is kept and panes can move between opened spaces without a restart.
  // (ephemeral automation panes are the one exception — they mount without activating the space.)
  const activated = useWorkspaces((s) => s.activatedIds);
  const activateWorkspace = useWorkspaces((s) => s.activateWorkspace);
  useEffect(() => {
    if (activeId) activateWorkspace(activeId);
  }, [activeId, activateWorkspace]);

  // ONE stable reference per handler (store actions + setState setters are stable, drag is a ref),
  // so memoized TerminalPanes don't re-render when a sibling is focused or a drag updates overId.
  const onPaneFocus = useCallback((sid: string) => setFocused(sid), [setFocused]);
  const onPaneClose = useCallback((wsId: string, sid: string) => void closeSession(wsId, sid), []);
  const onPaneToggleMax = useCallback(
    (wsId: string, sid: string) => toggleMaximized(wsId, sid),
    [toggleMaximized],
  );
  // tabbed slots aren't drag targets in v1 — their panes get inert grip handlers
  const noopGrip = useCallback(() => {}, []);
  const onGripDown = useCallback(
    (e: RPointerEvent<HTMLDivElement>, wsId: string, sid: string) => {
      if (e.button !== 0) return;
      e.currentTarget.setPointerCapture?.(e.pointerId);
      // measure the cell now, while it's still laid out — the ghost is sized from it, and the grab
      // offset is what keeps it under your cursor rather than jumping
      const cell = e.currentTarget.closest<HTMLElement>(".pane-cell");
      const r = cell?.getBoundingClientRect();
      drag.current = {
        id: sid,
        ws: wsId,
        sx: e.clientX,
        sy: e.clientY,
        active: false,
        gx: r ? e.clientX - r.left : 0,
        gy: r ? e.clientY - r.top : 0,
      };
      dragRect.current = r ? { w: r.width, h: r.height } : null;
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
        // read from the store rather than closing over `active`, so this handler keeps a stable
        // identity and memoized panes don't re-render when a drag starts
        const st = useWorkspaces.getState();
        const sess = st.workspaces.find((w) => w.id === d.ws)?.sessions.find((s) => s.id === d.id);
        const r = dragRect.current;
        // a full-size proxy would cover the drop targets you're aiming at, so shrink it and scale
        // the grab offset by the same factor to keep the same point under the cursor
        const k = r ? Math.min(GHOST_MAX_W / r.w, GHOST_MAX_H / r.h, 1) : 1;
        ghostK.current = k;
        setGhost({
          title: sess?.title || sess?.provider || "Pane",
          provider: sess?.provider ?? "terminal",
          folder: sess?.cwd?.split(/[\\/]/).filter(Boolean).pop(),
          w: r ? r.w * k : GHOST_MAX_W,
          h: r ? r.h * k : GHOST_MAX_H,
        });
        // place it before the first paint so it never flashes at the origin
        moveGhost(e.clientX, e.clientY, d);
      }
      dragPos.current = { x: e.clientX, y: e.clientY };
      if (dragRaf.current) return;
      dragRaf.current = requestAnimationFrame(() => {
        dragRaf.current = 0;
        const cur = drag.current;
        if (!cur?.active) return; // drag ended before the frame
        const { x, y } = dragPos.current;
        moveGhost(x, y, cur);
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
    [setDragId, setOverId, moveGhost], // moveGhost is a stable useCallback([]), so this stays constant
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
      setGhost(null);
      dragRect.current = null;
      useUi.getState().setPaneDrag(false);
    },
    [moveToWs, reorder, setDragId, setOverId],
  );

  const GhostIcon = ghost ? (PROVIDER_ICONS[ghost.provider] ?? TerminalIcon) : TerminalIcon;

  return (
    <div className={`pane-stage${dragId ? " dragging-active" : ""}`}>
      {/* The pane in your hand. position:fixed escapes the grid (no transformed ancestor), and
          pointer-events:none is load-bearing — the drop hit-test is elementFromPoint, so a ghost
          that could be hit would shadow every target underneath it. */}
      {ghost && (
        <div
          ref={ghostRef}
          className="pane-ghost"
          style={{ width: ghost.w, height: ghost.h }}
          aria-hidden
        >
          <div className="pane-ghost-head">
            <GhostIcon size={12} className="pane-ghost-ico" />
            <span className="pane-ghost-title">{ghost.title}</span>
            {ghost.folder && <span className="pane-ghost-cwd">· {ghost.folder}</span>}
          </div>
          <div className="pane-ghost-body" />
        </div>
      )}
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
        <div className="pane-empty-composer">
          <ComposerPane wsId={active.id} spacePicker />
        </div>
      )}

      {/* ONE grid holds every space's panes; inactive ones are display:none so their PTYs stay
          alive AND a pane can move between spaces without React remounting it — the key stays
          under the same parent, so the xterm + PTY survive the move instead of restarting. */}
      <div
        ref={gridRef}
        className={`pane-grid${maxedHere ? " maxed" : ""}`}
        style={{
          display: showGrid ? "grid" : "none",
          gridTemplateColumns: maxedHere ? "1fr" : gridCols,
          gridTemplateRows: maxedHere ? undefined : gridRows,
        }}
      >
        {workspaces.flatMap((w) => {
          // a space you haven't opened this run mounts nothing — EXCEPT ephemeral automation panes,
          // which must spawn to run; mounting just those avoids spawning every saved pane in it
          const sessions = activated.includes(w.id) ? w.sessions : w.sessions.filter((s) => s.ephemeral);
          if (!sessions.length) return [];
          const isActiveWs = w.id === activeId;
          const slots = toSlots(sessions);
          const layout = resolveLayout(slots.length, w.layouts?.[slots.length]);
          return slots.map((slot, si) => {
            const single = slot.sessions.length === 1; // drag/reorder is still per-pane
            const stored = slot.group ? w.activeTabByGroup?.[slot.group] : undefined;
            // fall back to the first pane if the stored active tab is gone
            const activeTab = stored && slot.sessions.some((ss) => ss.id === stored) ? stored : slot.sessions[0].id;
            // when maximized, only the slot holding the maximized pane stays on screen
            const wsMaxId = maximizedByWs[w.id]; // this row's workspace, not necessarily the active one
            const slotHasMax = maxedHere && slot.sessions.some((ss) => ss.id === wsMaxId);
            const cellVisible = isActiveWs && (!maxedHere || slotHasMax);
            const solo = slot.sessions[0];
            const place = layout.place(si);
            return (
              <div
                key={slot.group ?? solo.id}
                data-sid={single ? solo.id : undefined}
                className={`pane-cell tabbed${single && dragId === solo.id ? " dragging" : ""}${single && overId === solo.id ? " drop-over" : ""}${isActiveWs && focusedSessionId === activeTab ? " focused" : ""}`}
                style={{
                  display: cellVisible ? undefined : "none",
                  gridColumn: cellVisible && !maxedHere ? place.gridColumn : undefined,
                  gridRow: cellVisible && !maxedHere ? place.gridRow : undefined,
                }}
              >
                <div
                    className="pane-tabs"
                    onPointerDown={single ? (e) => {
                      // only the bar itself drags, not a tab or a button
                      if ((e.target as HTMLElement).closest(".pane-tab, button")) return;
                      onGripDown(e, w.id, solo.id);
                    } : undefined}
                    onPointerMove={single ? onGripMove : undefined}
                    onPointerUp={single ? onGripUp : undefined}
                    onDoubleClick={(e) => {
                      if ((e.target as HTMLElement).closest(".pane-tab, button")) return;
                      const act = slot.sessions.find((ss) => ss.id === activeTab) ?? solo;
                      onPaneToggleMax(w.id, act.id);
                    }}
                  >
                    {single && <GripVertical size={12} className="pane-grip" />}
                    {/* one pane = a plain title, no tab chrome. tabs only appear once a slot
                        actually stacks more than one pane (an image / file opened into it) */}
                    {single ? (
                      (() => {
                        const SIcon = PROVIDER_ICONS[solo.provider] ?? TerminalIcon;
                        const logo = PROVIDER_LOGO[solo.provider];
                        return (
                          <span
                            className="pane-solo"
                            title={solo.image || solo.file || solo.title || solo.provider}
                            onContextMenu={(e: RMouseEvent) => {
                              e.preventDefault();
                              e.stopPropagation();
                              setTabCtx({ x: e.clientX, y: e.clientY, wsId: w.id, sessionId: solo.id });
                            }}
                          >
                            {solo.draft ? null : logo ? (
                              <img className="pane-solo-logo" src={logo} alt="" />
                            ) : (
                              <SIcon size={12} className="pane-tab-ico" />
                            )}
                            <span className="pane-solo-title">{solo.title || solo.provider}</span>
                          </span>
                        );
                      })()
                    ) : (
                    slot.sessions.map((ts) => {
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
                    })
                    )}
                    {/* identity + controls for the ACTIVE tab — deliberately the same folder label
                        and the same .pane-btn group a solo pane's header shows, so a slot looks the
                        same whether or not it happens to be tabbed */}
                    <span className="pane-tabs-gap" />
                    {single && (
                      <span className="pane-head-right">
                        <button
                          className="pane-btn close"
                          title={wsMaxId === solo.id ? "Close pane (double-click the bar to restore)" : "Close pane (double-click the bar to maximize)"}
                          onClick={() => onPaneClose(w.id, solo.id)}
                        >
                          <X size={13} />
                        </button>
                      </span>
                    )}
                  </div>
                {slot.sessions.map((sess: Session) => {
                  // one pane is on screen per visible slot: the maximized pane when maximizing,
                  // else the slot's active tab. hidden tabs stay mounted (display:none) so their PTY lives.
                  // maximize pins the SLOT (via cellVisible/slotHasMax); the tab strip keeps
                  // choosing what shows inside it. Pinning to maximizedId here froze the body on
                  // the maximized session — ctrl+clicking an image opened + selected its tab while
                  // the terminal kept rendering, and tab clicks while maximized did nothing.
                  const visible = cellVisible && sess.id === activeTab;
                  // a "guest" pane sits in a project space but points at a different folder than the
                  // project (e.g. dragged in from an open space) — flag it so it's obvious at a glance
                  const guest = w.kind === "project" && (sess.cwd ?? w.cwd) !== w.cwd;
                  const pane = sess.draft ? (
                    <ComposerPane wsId={w.id} sessionId={sess.id} compact />
                  ) : sess.image ? (
                    <ImageViewer path={sess.image} active={visible} onClose={() => onPaneClose(w.id, sess.id)} tabbed />
                  ) : sess.media ? (
                    <MediaViewer path={sess.media} active={visible} />
                  ) : sess.diff ? (
                    <DiffViewer cwd={sess.cwd ?? w.cwd} path={sess.diff} active={visible} />
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
                        isMaxed={wsMaxId === sess.id}
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
                      // panes without a terminal take focus here; a terminal pane focuses itself
                      onMouseDown={sess.image || sess.file || sess.media || sess.diff || sess.draft ? () => onPaneFocus(sess.id) : undefined}
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
      {showGrid && !maxedHere && preset && colW && rowW && (
        <>
          {resizableBoundaries(preset, "col").map((b) => (
            <div
              key={`c${b}`}
              className="pane-gutter col"
              style={{ left: gutterPos(colW, b) }}
              onPointerDown={(e) => onGutterDown(e, "col", b)}
              onPointerMove={onGutterMove}
              onPointerUp={onGutterUp}
            />
          ))}
          {resizableBoundaries(preset, "row").map((b) => (
            <div
              key={`r${b}`}
              className="pane-gutter row"
              style={{ top: gutterPos(rowW, b) }}
              onPointerDown={(e) => onGutterDown(e, "row", b)}
              onPointerMove={onGutterMove}
              onPointerUp={onGutterUp}
            />
          ))}
        </>
      )}
      {tabCtx && <TabContextMenu ctx={tabCtx} onClose={() => setTabCtx(null)} />}
    </div>
  );
}
