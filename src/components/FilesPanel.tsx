import { useEffect, useState } from "react";
import { useWorkspaces } from "../stores/workspace";
import { useUi } from "../stores/ui";
import { listDir, revealPath, type DirEntry } from "../api";
import { joinPath } from "../lib/projects";
import { maybeAutostart } from "../lib/startup";
import { ChevronRight, Folder, File as FileIcon, RefreshCw } from "lucide-react";

const parentOf = (path: string) => path.replace(/[\\/][^\\/]+[\\/]?$/, "") || path;

function TreeNode({
  path,
  name,
  dir,
  depth,
  wsId,
}: {
  path: string;
  name: string;
  dir: boolean;
  depth: number;
  wsId?: string;
}) {
  const [open, setOpen] = useState(false);
  const [kids, setKids] = useState<DirEntry[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);

  const load = async () => {
    setLoading(true);
    setKids(await listDir(path).catch(() => []));
    setLoading(false);
  };
  const toggle = () => {
    if (!dir) return;
    if (!open && kids === null) void load();
    setOpen((o) => !o);
  };

  const openAsProject = () => {
    useWorkspaces.getState().addWorkspace(name, path);
    useUi.getState().goSpace();
    setMenu(null);
  };
  const newTerminalHere = () => {
    const ws = useWorkspaces.getState();
    const target = wsId ?? ws.activeId;
    if (target) {
      ws.setActive(target);
      ws.addSession(target, undefined, path);
      maybeAutostart(target);
    }
    useUi.getState().goSpace();
    setMenu(null);
  };
  const reveal = () => {
    void revealPath(dir ? path : parentOf(path)).catch(() => {});
    setMenu(null);
  };

  const pad = depth * 12 + 8;
  return (
    <>
      <button
        className="ft-row"
        style={{ paddingLeft: pad }}
        onClick={toggle}
        onContextMenu={(e) => {
          e.preventDefault();
          e.stopPropagation();
          setMenu({ x: e.clientX, y: e.clientY });
        }}
      >
        {dir ? (
          <ChevronRight size={13} className={`ft-twist${open ? " open" : ""}`} />
        ) : (
          <span className="ft-spacer" />
        )}
        {dir ? <Folder size={14} className="ft-ico" /> : <FileIcon size={14} className="ft-ico file" />}
        <span className="ft-name">{name}</span>
      </button>

      {open && dir && (
        <>
          {loading && (
            <div className="ft-dim" style={{ paddingLeft: pad + 19 }}>
              …
            </div>
          )}
          {kids?.map((k) => (
            <TreeNode
              key={k.name}
              path={joinPath(path, k.name)}
              name={k.name}
              dir={k.dir}
              depth={depth + 1}
              wsId={wsId}
            />
          ))}
          {kids && kids.length === 0 && (
            <div className="ft-dim" style={{ paddingLeft: pad + 19 }}>
              empty
            </div>
          )}
        </>
      )}

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
            {dir && (
              <button className="ctx-item" onClick={openAsProject}>
                Open as project
              </button>
            )}
            {dir && (
              <button className="ctx-item" onClick={newTerminalHere}>
                Open terminal here
              </button>
            )}
            <button className="ctx-item" onClick={reveal}>
              Reveal in Explorer
            </button>
          </div>
        </>
      )}
    </>
  );
}

// The lazy tree for one folder. Reused by the dock Files tab and the sidebar. `wsId` is the project
// that owns this folder, so "Open terminal here" targets and switches to it.
export function FileTree({
  cwd,
  refreshKey,
  wsId,
}: {
  cwd: string;
  refreshKey?: number;
  wsId?: string;
}) {
  const [entries, setEntries] = useState<DirEntry[] | null>(null);
  useEffect(() => {
    setEntries(null);
    listDir(cwd)
      .then(setEntries)
      .catch(() => setEntries([]));
  }, [cwd, refreshKey]);

  return (
    <div className="ft-tree">
      {entries === null ? (
        <div className="ft-dim" style={{ paddingLeft: 12 }}>
          Loading…
        </div>
      ) : entries.length === 0 ? (
        <div className="ft-dim" style={{ paddingLeft: 12 }}>
          Empty folder.
        </div>
      ) : (
        entries.map((e) => (
          <TreeNode key={e.name} path={joinPath(cwd, e.name)} name={e.name} dir={e.dir} depth={0} wsId={wsId} />
        ))
      )}
    </div>
  );
}

// The active project's file tree, in the dock.
export function FilesPanel() {
  const ws = useWorkspaces((s) => s.workspaces.find((w) => w.id === s.activeId) ?? null);
  const [refreshKey, setRefreshKey] = useState(0);

  if (!ws || !ws.cwd) {
    return <div className="ft-empty">Open a project (a folder) to browse its files.</div>;
  }

  return (
    <div className="ft">
      <div className="ft-head">
        <span className="ft-root" title={ws.cwd}>
          {ws.name}
        </span>
        <button className="ft-refresh" title="Refresh" onClick={() => setRefreshKey((k) => k + 1)}>
          <RefreshCw size={13} />
        </button>
      </div>
      <FileTree cwd={ws.cwd} refreshKey={refreshKey} wsId={ws.id} />
    </div>
  );
}
