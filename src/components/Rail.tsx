import { useState } from "react";
import { useWorkspaces, type Workspace } from "../stores/workspace";
import { useUi } from "../stores/ui";
import { pickFolder } from "../api";

export function Rail() {
  const workspaces = useWorkspaces((s) => s.workspaces);
  const activeId = useWorkspaces((s) => s.activeId);
  const setActive = useWorkspaces((s) => s.setActive);
  const addWorkspace = useWorkspaces((s) => s.addWorkspace);
  const addOpenSpace = useWorkspaces((s) => s.addOpenSpace);
  const removeWorkspace = useWorkspaces((s) => s.removeWorkspace);
  const renameWorkspace = useWorkspaces((s) => s.renameWorkspace);
  const collapsed = useUi((s) => s.railCollapsed);
  const toggleRail = useUi((s) => s.toggleRail);

  const [menu, setMenu] = useState<{ x: number; y: number; id: string } | null>(null);
  const [editing, setEditing] = useState<string | null>(null);

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
      className={`rail-item ${w.id === activeId ? "active" : ""}`}
      title={w.cwd || w.name}
      onClick={() => setActive(w.id)}
      onContextMenu={(e) => {
        e.preventDefault();
        e.stopPropagation();
        setMenu({ x: e.clientX, y: e.clientY, id: w.id });
      }}
    >
      {w.kind === "open" ? (
        <svg className="rail-ico" width="11" height="11" viewBox="0 0 12 12" style={{ color: w.color }}>
          <rect x="0.5" y="0.5" width="4.5" height="4.5" rx="1" fill="currentColor" />
          <rect x="7" y="0.5" width="4.5" height="4.5" rx="1" fill="currentColor" />
          <rect x="0.5" y="7" width="4.5" height="4.5" rx="1" fill="currentColor" />
          <rect x="7" y="7" width="4.5" height="4.5" rx="1" fill="currentColor" />
        </svg>
      ) : (
        <span className="dot" style={{ background: w.color }} />
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
      <div className="rail-top">
        <button
          className="rail-toggle"
          title={collapsed ? "Expand sidebar" : "Collapse sidebar"}
          onClick={toggleRail}
        >
          {collapsed ? "»" : "«"}
        </button>
      </div>
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
