import { useEffect, useReducer, useRef, useState } from "react";
import { useWorkspaces, type Workspace } from "../stores/workspace";
import { useUi } from "../stores/ui";
import { useActivity } from "../stores/activity";
import { useAuth } from "../stores/auth";
import { relTime } from "../lib/time";
import { revealPath } from "../api";
import { FileTree } from "./FilesPanel";
import {
  GripVertical,
  Folder,
  LayoutGrid,
  Settings as SettingsIcon,
  Plus,
  X,
  Search,
  ChevronLeft,
  ChevronRight,
} from "lucide-react";

const wsAt = (x: number, y: number): string | null => {
  const el = document.elementFromPoint(x, y) as HTMLElement | null;
  return el?.closest<HTMLElement>(".rail-item")?.dataset.wsid ?? null;
};

// the part of a session's cwd below its project folder (e.g. "spookeypumpkin27"), "" if it's the root
function relSub(wsCwd: string, sessCwd?: string): string {
  if (!sessCwd || !wsCwd) return "";
  const a = wsCwd.replace(/[\\/]+$/, "").toLowerCase();
  const b = sessCwd.replace(/[\\/]+$/, "");
  const bl = b.toLowerCase();
  if (bl === a) return "";
  if (bl.startsWith(a + "\\") || bl.startsWith(a + "/")) return b.slice(a.length + 1);
  return b.split(/[\\/]/).pop() || ""; // not under the project — just its name
}

export function Rail() {
  const workspaces = useWorkspaces((s) => s.workspaces);
  const activeId = useWorkspaces((s) => s.activeId);
  const setActive = useWorkspaces((s) => s.setActive);
  const addOpenSpace = useWorkspaces((s) => s.addOpenSpace);
  const removeWorkspace = useWorkspaces((s) => s.removeWorkspace);
  const renameWorkspace = useWorkspaces((s) => s.renameWorkspace);
  const reorderWorkspaces = useWorkspaces((s) => s.reorderWorkspaces);
  const collapsed = useUi((s) => s.railCollapsed);
  const toggleRail = useUi((s) => s.toggleRail);
  const paneDragging = useUi((s) => s.paneDragging);
  const paneDragOverWs = useUi((s) => s.paneDragOverWs);
  const view = useUi((s) => s.view);
  const goSpace = useUi((s) => s.goSpace);
  const setFocused = useWorkspaces((s) => s.setFocused);
  const focusedSessionId = useWorkspaces((s) => s.focusedSessionId);
  const exited = useActivity((s) => s.exited);
  const lastOut = useActivity((s) => s.lastOut);
  const user = useAuth((s) => s.user);
  const avatar =
    typeof user?.user_metadata?.avatar_url === "string" ? user.user_metadata.avatar_url : null;
  const acctName =
    ((user?.user_metadata?.full_name as string) || user?.email?.split("@")[0] || "Account").trim();
  const acctInitial = (acctName || "?")[0]?.toUpperCase() ?? "?";

  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [, tick] = useReducer((x) => x + 1, 0);
  const [menu, setMenu] = useState<{ x: number; y: number; id: string } | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const drag = useRef<{ id: string; sx: number; sy: number; active: boolean } | null>(null);
  const [dragId, setDragId] = useState<string | null>(null);
  const [overId, setOverId] = useState<string | null>(null);

  const projects = workspaces.filter((w) => w.kind !== "open");
  const openSpaces = workspaces.filter((w) => w.kind === "open");

  // auto-expand the active OPEN SPACE so its session list shows. projects aren't auto-expanded —
  // their chevron opens a file tree, and dumping that on every click is annoying (toggle it yourself).
  // while anything is expanded, re-render once a second so the "working" dots decay back to idle.
  useEffect(() => {
    if (!activeId) return;
    const ws = useWorkspaces.getState().workspaces.find((w) => w.id === activeId);
    if (ws?.kind === "open") setExpanded((p) => (p.has(activeId) ? p : new Set(p).add(activeId)));
  }, [activeId]);
  useEffect(() => {
    if (expanded.size === 0) return;
    const t = setInterval(tick, 1000);
    return () => clearInterval(t);
  }, [expanded]);

  const toggleExpand = (id: string) =>
    setExpanded((p) => {
      const n = new Set(p);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });
  const focusSession = (wid: string, sid: string) => {
    setActive(wid);
    setFocused(sid);
    goSpace();
  };
  const sessDot = (id: string): "busy" | "ready" | "exited" => {
    if (exited[id]) return "exited";
    const t = lastOut[id];
    return t && Date.now() - t < 1500 ? "busy" : "ready";
  };

  const item = (w: Workspace) => {
    const isExpanded = expanded.has(w.id);
    const hasSessions = w.sessions.length > 0;
    // projects can expand to browse their folders even with no sessions yet
    const canExpand = hasSessions || (w.kind !== "open" && !!w.cwd);
    return (
      <div key={w.id} className="rail-item-wrap" data-wsid={w.id}>
        <div
          data-wsid={w.id}
          className={`rail-item ${w.id === activeId && view === "space" ? "active" : ""}${
            dragId === w.id ? " dragging" : ""
          }${overId === w.id ? " drop-over" : ""}${
            paneDragging && w.id !== activeId ? " pane-droppable" : ""
          }${paneDragOverWs === w.id ? " pane-drop-over" : ""}`}
          title={w.cwd || w.name}
          onClick={() => {
            setActive(w.id);
            goSpace();
          }}
          onDoubleClick={() => setEditing(w.id)}
          onContextMenu={(e) => {
            e.preventDefault();
            e.stopPropagation();
            setMenu({ x: e.clientX, y: e.clientY, id: w.id });
          }}
        >
          {canExpand ? (
            <button
              className={`rail-twist${isExpanded ? " open" : ""}`}
              title={isExpanded ? "Collapse" : "Expand"}
              onClick={(e) => {
                e.stopPropagation();
                toggleExpand(w.id);
              }}
            >
              <ChevronRight size={13} />
            </button>
          ) : (
            <span className="rail-twist-spacer" />
          )}
          <span
            className="rail-grip"
            title="Drag to reorder"
            onClick={(e) => e.stopPropagation()}
            onPointerDown={(e) => {
              e.stopPropagation();
              e.currentTarget.setPointerCapture?.(e.pointerId);
              drag.current = { id: w.id, sx: e.clientX, sy: e.clientY, active: false };
            }}
            onPointerMove={(e) => {
              const d = drag.current;
              if (!d) return;
              if (!d.active) {
                if (Math.hypot(e.clientX - d.sx, e.clientY - d.sy) < 4) return;
                d.active = true;
                setDragId(d.id);
              }
              const t = wsAt(e.clientX, e.clientY);
              setOverId(t && t !== d.id ? t : null);
            }}
            onPointerUp={(e) => {
              const d = drag.current;
              drag.current = null;
              e.currentTarget.releasePointerCapture?.(e.pointerId);
              if (d?.active) {
                const t = wsAt(e.clientX, e.clientY);
                if (t && t !== d.id) reorderWorkspaces(d.id, t);
              }
              setDragId(null);
              setOverId(null);
            }}
          >
            <GripVertical size={13} aria-hidden="true" />
          </span>
          {w.kind === "open" ? (
            <LayoutGrid className="rail-ico" size={14} style={{ color: w.color }} />
          ) : (
            <Folder className="rail-ico" size={14} style={{ color: w.color }} />
          )}
          {editing === w.id ? (
            <input
              className="rail-rename"
              autoFocus
              defaultValue={w.name}
              onClick={(e) => e.stopPropagation()}
              onBlur={(e) => {
                const v = e.currentTarget.value.trim();
                if (v) renameWorkspace(w.id, v);
                setEditing(null);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") e.currentTarget.blur();
                if (e.key === "Escape") setEditing(null);
              }}
            />
          ) : (
            <span className="rail-name">{w.name}</span>
          )}
          {hasSessions && <span className="rail-count">{w.sessions.length}</span>}
          <button
            className="rail-clear"
            title={`Clear "${w.name}" (closes its sessions — your files stay)`}
            onClick={(e) => {
              e.stopPropagation();
              removeWorkspace(w.id);
            }}
          >
            <X size={12} />
          </button>
        </div>
        {isExpanded && !collapsed && (
          <>
            {hasSessions && (
              <div className="rail-sessions">
                {w.sessions.map((s) => {
                  const dot = sessDot(s.id);
                  const active = view === "space" && w.id === activeId && focusedSessionId === s.id;
                  const sub = relSub(w.cwd, s.cwd); // subfolder it's running in, if any
                  return (
                    <button
                      key={s.id}
                      className={`rail-session${active ? " active" : ""}`}
                      title={s.cwd || s.title}
                      onClick={() => focusSession(w.id, s.id)}
                    >
                      <span className={`rail-sess-dot s-${dot}`} />
                      <span className="rail-sess-name">{s.title}</span>
                      {sub && (
                        <span className="rail-sess-sub" title={s.cwd}>
                          {sub}
                        </span>
                      )}
                      {lastOut[s.id] ? (
                        <span className="rail-sess-time">{relTime(lastOut[s.id])}</span>
                      ) : null}
                    </button>
                  );
                })}
              </div>
            )}
            {w.kind !== "open" && w.cwd && (
              <div className="rail-files">
                <FileTree cwd={w.cwd} wsId={w.id} />
              </div>
            )}
          </>
        )}
      </div>
    );
  };

  return (
    <div className={`rail${collapsed ? " collapsed" : ""}`}>
      <button
        className="rail-edge"
        title={collapsed ? "Expand sidebar" : "Collapse sidebar"}
        onClick={toggleRail}
      >
        {collapsed ? <ChevronRight size={15} /> : <ChevronLeft size={15} />}
      </button>
      <div className="rail-scroll">
      <button className="rail-search" onClick={() => useUi.getState().setPalette(true)}>
        <Search size={15} />
        <span className="rail-search-label">Search</span>
        <span className="rail-search-kbd">Ctrl K</span>
      </button>
      <div className="rail-header">
        <span className="rail-title">PROJECTS</span>
        <button
          className="rail-add"
          title="New project"
          onClick={() => useUi.getState().openNewProject()}
        >
          <Plus size={15} />
        </button>
      </div>
      <div className="rail-list">{projects.map(item)}</div>

      <div className="rail-header">
        <span className="rail-title">OPEN SPACES</span>
        <button
          className="rail-add"
          title="New open space"
          onClick={() => {
            addOpenSpace();
            goSpace();
          }}
        >
          <Plus size={15} />
        </button>
      </div>
      <div className="rail-list">
        {openSpaces.map(item)}
        {openSpaces.length === 0 && <div className="rail-empty">launch sessions in any folder</div>}
      </div>
      </div>

      <div className="rail-foot">
        <button
          className="rail-acct"
          title={user?.email ?? "Account"}
          onClick={() => useUi.getState().openSettings("account")}
        >
          {avatar ? (
            <img className="rail-acct-ava" src={avatar} alt="" referrerPolicy="no-referrer" />
          ) : (
            <span className="rail-acct-ava rail-acct-fallback">{acctInitial}</span>
          )}
          <span className="rail-acct-meta">
            <span className="rail-acct-name">{acctName}</span>
            <span className="rail-acct-sub">{user?.email ?? "Signed in"}</span>
          </span>
        </button>
        <button
          className="rail-foot-btn"
          title="Settings"
          onClick={() => useUi.getState().openSettings()}
        >
          <SettingsIcon size={16} strokeWidth={1.75} />
        </button>
      </div>

      {menu && (
        <>
          <div
            className="ctx-backdrop"
            onClick={() => setMenu(null)}
            onContextMenu={(e) => {
              e.preventDefault();
              setMenu(null);
            }}
          />
          <div className="ctx-menu" style={{ left: menu.x, top: menu.y }}>
            {workspaces.find((w) => w.id === menu.id)?.cwd && (
              <button
                className="ctx-item"
                onClick={() => {
                  const w = workspaces.find((x) => x.id === menu.id);
                  if (w?.cwd) void revealPath(w.cwd).catch(() => {});
                  setMenu(null);
                }}
              >
                Open folder
              </button>
            )}
            {workspaces.find((w) => w.id === menu.id)?.cwd && (
              <button
                className="ctx-item"
                onClick={() => {
                  const w = workspaces.find((x) => x.id === menu.id);
                  if (w?.cwd) useUi.getState().openServices({ folder: w.cwd, wsId: w.id, name: w.name });
                  setMenu(null);
                }}
              >
                Services
              </button>
            )}
            <button
              className="ctx-item"
              onClick={() => {
                setEditing(menu.id);
                setMenu(null);
              }}
            >
              Rename
            </button>
            <button
              className="ctx-item danger"
              onClick={() => {
                removeWorkspace(menu.id);
                setMenu(null);
              }}
            >
              Clear
            </button>
          </div>
        </>
      )}
    </div>
  );
}
