import { useEffect, useMemo, useState } from "react";
import { useAutoAnimate } from "@formkit/auto-animate/react";
import { useWorkspaces } from "../stores/workspace";
import { useUi } from "../stores/ui";
import { listDir, revealPath, type DirEntry } from "../api";
import { joinPath } from "../lib/projects";
import { maybeAutostart } from "../lib/startup";
import { claudeCmd, geminiCmd, codexCmd } from "../actions";
import { isWindows } from "../platform";
import { ChevronRight, Folder, File as FileIcon, RefreshCw } from "lucide-react";

const WSL_CMD = "wsl";

const parentOf = (path: string) => path.replace(/[\\/][^\\/]+[\\/]?$/, "") || path;

const normPath = (p: string) => p.replace(/[\\/]+$/, "").toLowerCase();

function TreeNode({
  path,
  name,
  dir,
  depth,
  wsId,
  liveDirs,
}: {
  path: string;
  name: string;
  dir: boolean;
  depth: number;
  wsId?: string;
  liveDirs?: Set<string>; // normalized cwds that have an open session — so we can flag them
}) {
  const [open, setOpen] = useState(false);
  const [kids, setKids] = useState<DirEntry[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const [aaRef] = useAutoAnimate(); // smooth folder expand/collapse

  const load = async () => {
    setLoading(true);
    setKids(await listDir(path).catch(() => []));
    setLoading(false);
  };
  const toggle = () => {
    if (!dir) {
      useUi.getState().openInEditor(path); // click a file → open it in the editor tab
      return;
    }
    if (!open && kids === null) void load();
    setOpen((o) => !o);
  };

  const openAsProject = () => {
    useWorkspaces.getState().addWorkspace(name, path);
    useUi.getState().goSpace();
    setMenu(null);
  };
  // launch a pane in this folder — a provider (claude/gemini/…) or a plain terminal (no command)
  const launchHere = (command?: string) => {
    const ws = useWorkspaces.getState();
    const target = wsId ?? ws.activeId;
    if (target) {
      ws.setActive(target);
      ws.addSession(target, command, path);
      maybeAutostart(target);
    }
    useUi.getState().goSpace();
    setMenu(null);
  };
  const launchers: { label: string; cmd?: string }[] = [
    { label: "Open Claude here", cmd: claudeCmd() },
    { label: "Open Gemini here", cmd: geminiCmd() },
    { label: "Open Codex here", cmd: codexCmd() },
    ...(isWindows ? [{ label: "Open WSL here", cmd: WSL_CMD }] : []),
    { label: "Open terminal here", cmd: undefined },
  ];
  const reveal = () => {
    void revealPath(dir ? path : parentOf(path)).catch(() => {});
    setMenu(null);
  };

  const pad = depth * 12 + 8;
  return (
    <div className="ft-node" ref={aaRef}>
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
        {dir && liveDirs?.has(normPath(path)) && (
          <span className="ft-live" title="A session is running here" />
        )}
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
              liveDirs={liveDirs}
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
            {!dir && (
              <button
                className="ctx-item"
                onClick={() => {
                  useUi.getState().openInEditor(path);
                  setMenu(null);
                }}
              >
                Open in editor
              </button>
            )}
            {dir && (
              <button className="ctx-item" onClick={openAsProject}>
                Open as project
              </button>
            )}
            {dir &&
              launchers.map((l) => (
                <button key={l.label} className="ctx-item" onClick={() => launchHere(l.cmd)}>
                  {l.label}
                </button>
              ))}
            <button className="ctx-item" onClick={reveal}>
              Reveal in Explorer
            </button>
          </div>
        </>
      )}
    </div>
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
  // folders (normalized cwds) that have an open session in this project — to flag them in the tree.
  // select the stable sessions array, then memo the Set (a fresh Set per render breaks the store snapshot)
  const sessions = useWorkspaces((s) => s.workspaces.find((w) => w.id === wsId)?.sessions);
  const liveDirs = useMemo(
    () => new Set((sessions ?? []).map((x) => normPath(x.cwd ?? "")).filter(Boolean)),
    [sessions],
  );
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
          <TreeNode
            key={e.name}
            path={joinPath(cwd, e.name)}
            name={e.name}
            dir={e.dir}
            depth={0}
            wsId={wsId}
            liveDirs={liveDirs}
          />
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
