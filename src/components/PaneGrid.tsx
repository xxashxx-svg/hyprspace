import { useCallback, useRef, useState } from "react";
import type { PointerEvent as RPointerEvent } from "react";
import { useWorkspaces } from "../stores/workspace";
import { useUi } from "../stores/ui";
import { useProjectConfigs, folderKey } from "../stores/projectConfig";
import { launchTask, maybeAutostart } from "../lib/startup";
import { useServices, serviceId } from "../stores/services";
import { TerminalPane } from "./TerminalPane";
import { Logo } from "./Logo";
import { pickFolders } from "../api";
import { closeSession, claudeCmd, geminiCmd, codexCmd } from "../actions";
import { isWindows } from "../platform";
import { Play, ScrollText } from "lucide-react";

const WSL_CMD = "wsl";

// configured services for the open project, as quick Run chips (so they're discoverable on open).
// background services show a live dot + open their logs on click instead of spawning a pane.
function EmptyServices({ wsId, folder }: { wsId: string; folder: string }) {
  const cfg = useProjectConfigs((s) => s.configs[folderKey(folder)]);
  const tasks = cfg?.startup ?? [];
  const running = useServices((s) => s.running);
  const known = useServices((s) => s.known);
  if (tasks.length === 0) return null;
  const openLogs = (t: { id: string; name: string }) =>
    useUi.getState().openServiceLogs({ id: serviceId(t.id), name: t.name || "service" });
  return (
    <div className="empty-services">
      <span className="empty-services-label">Services</span>
      <div className="empty-services-row">
        {tasks.map((t) => {
          if (!t.background) {
            return (
              <button key={t.id} className="empty-svc-chip" onClick={() => launchTask(wsId, t)}>
                <Play size={11} />
                {t.name || "service"}
              </button>
            );
          }
          const sid = serviceId(t.id);
          const on = !!running[sid];
          const hasLogs = on || !!known[sid];
          return (
            <div className={`empty-svc-chip bg${on ? " on" : ""}`} key={t.id}>
              <button
                className="empty-svc-main"
                title={on ? "Running in background — view logs" : "Run in background"}
                onClick={() => (on ? openLogs(t) : launchTask(wsId, t))}
              >
                {on ? <span className="svc-dot on" /> : <Play size={11} />}
                {t.name || "service"}
              </button>
              {hasLogs && (
                <button className="empty-svc-logs" title="View logs" onClick={() => openLogs(t)}>
                  <ScrollText size={11} />
                </button>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

// Tile like the Electron version did: keep rows balanced so a partial last row fills
// the width instead of leaving a hole (the old 5-pane "3 over a gap"). For 5+ we lay
// 3 panes per row on a 6-col grid and let a short last row span wider:
//   last row of 1 → full width, of 2 → halves, of 3 → thirds.
type GridLayout = { cols: string; span: (i: number) => string | undefined };

function getLayout(n: number): GridLayout {
  if (n <= 1) return { cols: "1fr", span: () => undefined };
  if (n === 2) return { cols: "1fr 1fr", span: () => undefined };
  if (n === 3) return { cols: "1fr 1fr 1fr", span: () => undefined };
  if (n === 4) return { cols: "1fr 1fr", span: () => undefined };
  return {
    cols: "repeat(6, 1fr)",
    span: (i) => {
      const rem = n % 3;
      const lastRowStart = n - (rem === 0 ? 3 : rem);
      if (i < lastRowStart) return "span 2"; // full rows: three panes, 2 cols each
      const inLast = n - lastRowStart;
      return inLast === 1 ? "span 6" : inLast === 2 ? "span 3" : "span 2";
    },
  };
}

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
  const addSession = useWorkspaces((s) => s.addSession);

  const maximizedId = useUi((s) => s.maximizedId);
  const toggleMaximized = useUi((s) => s.toggleMaximized);
  const fileDropId = useUi((s) => s.fileDropId);
  const skillDropId = useUi((s) => s.skillDropId);

  const drag = useRef<{ id: string; ws: string; sx: number; sy: number; active: boolean } | null>(
    null,
  );
  const [dragId, setDragId] = useState<string | null>(null);
  const [overId, setOverId] = useState<string | null>(null);

  const active = workspaces.find((w) => w.id === activeId) ?? null;
  const maxedHere = !!maximizedId && !!active && active.sessions.some((s) => s.id === maximizedId);
  const activeLayout = getLayout(active?.sessions.length ?? 0);
  const showGrid = !!active && active.sessions.length > 0;

  const launch = async (command?: string) => {
    if (!active) return;
    if (active.kind === "open") {
      const folders = await pickFolders();
      folders.forEach((f) => addSession(active.id, command, f));
    } else {
      addSession(active.id, command);
    }
    maybeAutostart(active.id);
  };

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
      // hovering a different space in the rail → it becomes the drop target
      const overWs = railWsAt(e.clientX, e.clientY);
      if (overWs && overWs !== d.ws) {
        setOverId(null);
        useUi.getState().setPaneDragOverWs(overWs);
      } else {
        useUi.getState().setPaneDragOverWs(null);
        const sid = cellSidAt(e.clientX, e.clientY);
        setOverId(sid && sid !== d.id ? sid : null);
      }
    },
    [setDragId, setOverId],
  );
  const onGripUp = useCallback(
    (e: RPointerEvent<HTMLDivElement>) => {
      const d = drag.current;
      drag.current = null;
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
        <div className="pane-empty">
          <div className="empty-logo">
            <Logo size={28} />
          </div>
          <div className="empty-title">{active.name}</div>
          <div className="empty-hint">
            {active.kind === "open"
              ? "Launch an AI assistant or a terminal in any folder — pick one or several"
              : "Launch a terminal or an AI assistant to start working"}
          </div>
          <div className="empty-actions">
            <button className="launch-btn launch-primary" onClick={() => launch(claudeCmd())}>
              Claude
            </button>
            <button className="launch-btn" onClick={() => launch(geminiCmd())}>
              Gemini
            </button>
            <button className="launch-btn" onClick={() => launch(codexCmd())}>
              Codex
            </button>
            {isWindows && (
              <button className="launch-btn" onClick={() => launch(WSL_CMD)}>
                WSL
              </button>
            )}
            <button className="launch-btn" onClick={() => launch()}>
              Terminal
            </button>
          </div>
          {active.kind !== "open" && active.cwd && (
            <EmptyServices wsId={active.id} folder={active.cwd} />
          )}
        </div>
      )}

      {/* ONE grid holds every space's panes; inactive ones are display:none so their PTYs stay
          alive AND a pane can move between spaces without React remounting it — the key stays
          under the same parent, so the xterm + PTY survive the move instead of restarting. */}
      <div
        className={`pane-grid${maxedHere ? " maxed" : ""}`}
        style={{
          display: showGrid ? "grid" : "none",
          gridTemplateColumns: maxedHere ? "1fr" : activeLayout.cols,
        }}
      >
        {workspaces.flatMap((w) => {
          const isActiveWs = w.id === activeId;
          const layout = getLayout(w.sessions.length);
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
                  gridColumn: visible && !maxedHere ? layout.span(i) : undefined,
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
