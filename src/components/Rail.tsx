import { useRef, useState } from "react";
import { useWorkspaces, type Workspace } from "../stores/workspace";
import { useUi } from "../stores/ui";
import { pickFolder } from "../api";

const wsAt = (x: number, y: number): string | null => {
  const el = document.elementFromPoint(x, y) as HTMLElement | null;
  return el?.closest<HTMLElement>(".rail-item")?.dataset.wsid ?? null;
};

export function Rail() {
  const workspaces = useWorkspaces((s) => s.workspaces);
  const activeId = useWorkspaces((s) => s.activeId);
  const setActive = useWorkspaces((s) => s.setActive);
  const addWorkspace = useWorkspaces((s) => s.addWorkspace);
  const addOpenSpace = useWorkspaces((s) => s.addOpenSpace);
  const removeWorkspace = useWorkspaces((s) => s.removeWorkspace);
  const renameWorkspace = useWorkspaces((s) => s.renameWorkspace);
  const reorderWorkspaces = useWorkspaces((s) => s.reorderWorkspaces);
  const collapsed = useUi((s) => s.railCollapsed);
  const toggleRail = useUi((s) => s.toggleRail);

  const [menu, setMenu] = useState<{ x: number; y: number; id: string } | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const drag = useRef<{ id: string; sx: number; sy: number; active: boolean } | null>(null);
  const [dragId, setDragId] = useState<string | null>(null);
  const [overId, setOverId] = useState<string | null>(null);

  const projects = workspaces.filter((w) => w.kind !== "open");
  const openSpaces = workspaces.filter((w) => w.kind === "open");

  const newProject = async () => {
    const folder = await pickFolder();
    if (!folder) return;
    const name = folder.split(/[\\/]/).filter(Boolean).pop() || "Project";
    addWorkspace(name, folder);
  };

  const item = (w: Workspace) => (
    <div
      key={w.id}
      data-wsid={w.id}
      className={`rail-item ${w.id === activeId ? "active" : ""}${dragId === w.id ? " dragging" : ""}${
        overId === w.id ? " drop-over" : ""
      }`}
      title={w.cwd || w.name}
      onClick={() => setActive(w.id)}
      onDoubleClick={() => setEditing(w.id)}
      onContextMenu={(e) => {
        e.preventDefault();
        e.stopPropagation();
        setMenu({ x: e.clientX, y: e.clientY, id: w.id });
      }}
    >
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
        <svg width="6" height="13" viewBox="0 0 6 13" fill="currentColor" aria-hidden="true">
          <circle cx="1.5" cy="2.5" r="0.9" />
          <circle cx="4.5" cy="2.5" r="0.9" />
          <circle cx="1.5" cy="6.5" r="0.9" />
          <circle cx="4.5" cy="6.5" r="0.9" />
          <circle cx="1.5" cy="10.5" r="0.9" />
          <circle cx="4.5" cy="10.5" r="0.9" />
        </svg>
      </span>
      {w.kind === "open" ? (
        <svg className="rail-ico" width="12" height="12" viewBox="0 0 12 12" style={{ color: w.color }}>
          <rect x="0.5" y="0.5" width="4.5" height="4.5" rx="1" fill="currentColor" />
          <rect x="7" y="0.5" width="4.5" height="4.5" rx="1" fill="currentColor" />
          <rect x="0.5" y="7" width="4.5" height="4.5" rx="1" fill="currentColor" />
          <rect x="7" y="7" width="4.5" height="4.5" rx="1" fill="currentColor" />
        </svg>
      ) : (
        <svg className="rail-ico" width="13" height="13" viewBox="0 0 14 14" style={{ color: w.color }}>
          <path
            d="M1 4a1.2 1.2 0 0 1 1.2-1.2h2.4l1.2 1.3H11.8A1.2 1.2 0 0 1 13 5.3V11a1.2 1.2 0 0 1-1.2 1.2H2.2A1.2 1.2 0 0 1 1 11V4z"
            fill="currentColor"
          />
        </svg>
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
      {w.sessions.length > 0 && <span className="rail-count">{w.sessions.length}</span>}
      <button
        className="rail-clear"
        title={`Clear "${w.name}" (closes its sessions — your files stay)`}
        onClick={(e) => {
          e.stopPropagation();
          removeWorkspace(w.id);
        }}
      >
        ×
      </button>
    </div>
  );

  return (
    <div className={`rail${collapsed ? " collapsed" : ""}`}>
      <button
        className="rail-edge"
        title={collapsed ? "Expand sidebar" : "Collapse sidebar"}
        onClick={toggleRail}
      >
        {collapsed ? "›" : "‹"}
      </button>
      <div className="rail-scroll">
      <div className="rail-header">
        <span className="rail-title">PROJECTS</span>
        <button className="rail-add" title="Open a folder as a project" onClick={newProject}>
          +
        </button>
      </div>
      <div className="rail-list">{projects.map(item)}</div>

      <div className="rail-header">
        <span className="rail-title">OPEN SPACES</span>
        <button className="rail-add" title="New open space" onClick={() => addOpenSpace()}>
          +
        </button>
      </div>
      <div className="rail-list">
        {openSpaces.map(item)}
        {openSpaces.length === 0 && <div className="rail-empty">launch sessions in any folder</div>}
      </div>
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
