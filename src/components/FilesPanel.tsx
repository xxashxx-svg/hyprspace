import { useEffect, useMemo, useRef, useState } from "react";
import { useAutoAnimate } from "@formkit/auto-animate/react";
import { useWorkspaces } from "../stores/workspace";
import { useUi } from "../stores/ui";
import { listDir, revealPath, fileOp, type DirEntry } from "../api";
import { joinPath } from "../lib/projects";
import { maybeAutostart } from "../lib/startup";
import { confirmDialog } from "../stores/confirm";
import { claudeCmd, geminiCmd, codexCmd, opencodeCmd, grokCmd } from "../actions";
import { isWindows } from "../platform";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { ChevronRight, Folder, File as FileIcon, RefreshCw } from "lucide-react";

const WSL_CMD = "wsl";

const parentOf = (path: string) => path.replace(/[\\/][^\\/]+[\\/]?$/, "") || path;

const normPath = (p: string) => p.replace(/[\\/]+$/, "").toLowerCase();

// shared dir-listing cache (module-level) so reopening a folder — or re-expanding the sidebar —
// paints instantly from cache instead of flashing "Loading…" then popping to full height. survives
// unmounts, dedupes in-flight fetches.
const dirCache = new Map<string, DirEntry[]>();
const dirPending = new Map<string, Promise<DirEntry[]>>();
export function peekDir(path: string): DirEntry[] | null {
  return dirCache.get(normPath(path)) ?? null;
}
export function loadDir(path: string, force = false): Promise<DirEntry[]> {
  const key = normPath(path);
  if (!force) {
    const cached = dirCache.get(key);
    if (cached) return Promise.resolve(cached);
    const inflight = dirPending.get(key);
    if (inflight) return inflight;
  }
  const p = listDir(path)
    .catch(() => [] as DirEntry[])
    .then((e) => {
      dirCache.set(key, e);
      dirPending.delete(key);
      return e;
    });
  dirPending.set(key, p);
  return p;
}
export function invalidateDir(path: string) {
  dirCache.delete(normPath(path));
}

function TreeNode({
  path,
  name,
  dir,
  depth,
  wsId,
  liveDirs,
  rootCwd,
  onChanged,
}: {
  path: string;
  name: string;
  dir: boolean;
  depth: number;
  wsId?: string;
  liveDirs?: Set<string>; // normalized cwds that have an open session — so we can flag them
  rootCwd: string; // the tree's root, for "Copy relative path"
  onChanged: () => void; // parent re-lists after a rename/delete of this node
}) {
  const [open, setOpen] = useState(false);
  const [kids, setKids] = useState<DirEntry[] | null>(() => peekDir(path));
  const [loading, setLoading] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const [renaming, setRenaming] = useState(false);
  const [creating, setCreating] = useState<"file" | "dir" | null>(null); // input row for a new child
  const [aaRef] = useAutoAnimate(); // smooth folder expand/collapse

  // re-list THIS dir's children (used after creating a child, and passed down to kids)
  const reloadKids = async () => {
    invalidateDir(path);
    setKids(await loadDir(path, true));
  };

  const load = async () => {
    setLoading(true);
    setKids(await loadDir(path));
    setLoading(false);
  };
  const toggle = async () => {
    if (!dir) {
      useUi.getState().openInEditor(path); // click a file → open it in the editor tab
      return;
    }
    if (open) {
      setOpen(false);
      return;
    }
    if (kids === null) await load(); // fetch BEFORE revealing so it opens straight to full height (no pop)
    setOpen(true);
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
    { label: "Open OpenCode here", cmd: opencodeCmd() },
    { label: "Open Grok here", cmd: grokCmd() },
    ...(isWindows ? [{ label: "Open WSL here", cmd: WSL_CMD }] : []),
    { label: "Open terminal here", cmd: undefined },
  ];
  const reveal = () => {
    void revealPath(dir ? path : parentOf(path)).catch(() => {});
    setMenu(null);
  };

  // ---- file ops (Orca-style context menu) ----
  const relOf = (p: string) => {
    const root = rootCwd.replace(/[\\/]+$/, "");
    return p.startsWith(root) ? p.slice(root.length + 1) : name;
  };
  const copyPath = (rel: boolean) => {
    void writeText(rel ? relOf(path) : path).catch(() => {});
    setMenu(null);
  };
  const startCreate = async (kind: "file" | "dir") => {
    setMenu(null);
    if (kids === null) await load();
    setOpen(true);
    setCreating(kind);
  };
  const doDelete = async () => {
    setMenu(null);
    const ok = await confirmDialog({
      title: dir ? "Delete folder?" : "Delete file?",
      message: `"${name}" will be permanently deleted${dir ? ", including everything inside" : ""}.`,
      confirmLabel: "Delete",
      danger: true,
    });
    if (!ok) return;
    await fileOp("delete", path).catch(() => {});
    onChanged();
  };
  const commitRename = async (next: string) => {
    setRenaming(false);
    const v = next.trim();
    if (!v || v === name || /[\\/]/.test(v)) return;
    await fileOp("rename", path, joinPath(parentOf(path), v)).catch(() => {});
    onChanged();
  };
  const commitCreate = async (next: string) => {
    const kind = creating;
    setCreating(null);
    const v = next.trim();
    if (!kind || !v || /[\\/]/.test(v)) return;
    await fileOp(kind === "dir" ? "create-dir" : "create-file", joinPath(path, v)).catch(() => {});
    await reloadKids();
  };

  const pad = depth * 12 + 8;
  return (
    <div className="ft-node" ref={aaRef}>
      {renaming ? (
        <div className="ft-row" style={{ paddingLeft: pad }}>
          {dir ? (
            <ChevronRight size={13} className={`ft-twist${open ? " open" : ""}`} />
          ) : (
            <span className="ft-spacer" />
          )}
          {dir ? <Folder size={14} className="ft-ico" /> : <FileIcon size={14} className="ft-ico file" />}
          <input
            className="ft-rename"
            autoFocus
            defaultValue={name}
            onFocus={(e) => e.currentTarget.select()}
            onKeyDown={(e) => {
              if (e.key === "Enter") void commitRename(e.currentTarget.value);
              if (e.key === "Escape") setRenaming(false);
            }}
            onBlur={() => setRenaming(false)}
          />
        </div>
      ) : (
        <button
          className="ft-row"
          style={{ paddingLeft: pad }}
          onClick={() => void toggle()}
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
      )}

      {open && dir && (
        <>
          {loading && (
            <div className="ft-dim" style={{ paddingLeft: pad + 19 }}>
              …
            </div>
          )}
          {creating && (
            <div className="ft-row" style={{ paddingLeft: pad + 12 + 8 }}>
              {creating === "dir" ? (
                <Folder size={14} className="ft-ico" />
              ) : (
                <FileIcon size={14} className="ft-ico file" />
              )}
              <input
                className="ft-rename"
                autoFocus
                placeholder={creating === "dir" ? "folder name" : "file name"}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void commitCreate(e.currentTarget.value);
                  if (e.key === "Escape") setCreating(null);
                }}
                onBlur={() => setCreating(null)}
              />
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
              rootCwd={rootCwd}
              onChanged={() => void reloadKids()}
            />
          ))}
          {kids && kids.length === 0 && !creating && (
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
              <>
                <button className="ctx-item" onClick={() => void startCreate("file")}>
                  New file…
                </button>
                <button className="ctx-item" onClick={() => void startCreate("dir")}>
                  New folder…
                </button>
              </>
            )}
            <div className="ctx-sep" />
            <button className="ctx-item" onClick={() => copyPath(false)}>
              Copy path
            </button>
            <button className="ctx-item" onClick={() => copyPath(true)}>
              Copy relative path
            </button>
            <div className="ctx-sep" />
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
            <div className="ctx-sep" />
            <button
              className="ctx-item"
              onClick={() => {
                setMenu(null);
                setRenaming(true);
              }}
            >
              Rename
            </button>
            <button className="ctx-item danger" onClick={() => void doDelete()}>
              Delete
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
  const [entries, setEntries] = useState<DirEntry[] | null>(() => peekDir(cwd));
  // folders (normalized cwds) that have an open session in this project — to flag them in the tree.
  // select the stable sessions array, then memo the Set (a fresh Set per render breaks the store snapshot)
  const sessions = useWorkspaces((s) => s.workspaces.find((w) => w.id === wsId)?.sessions);
  const liveDirs = useMemo(
    () => new Set((sessions ?? []).map((x) => normPath(x.cwd ?? "")).filter(Boolean)),
    [sessions],
  );
  const lastCwd = useRef<string | null>(null);
  useEffect(() => {
    const cwdChanged = lastCwd.current !== cwd;
    lastCwd.current = cwd;
    // switching projects: show this folder's cache (or null→Loading). a refreshKey bump (same cwd)
    // forces a fresh read but keeps the stale tree on screen meanwhile — no Loading flash, no pop.
    if (cwdChanged) setEntries(peekDir(cwd));
    const force = !cwdChanged;
    if (force) invalidateDir(cwd);
    let alive = true;
    loadDir(cwd, force).then((e) => {
      if (alive) setEntries(e);
    });
    return () => {
      alive = false;
    };
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
            rootCwd={cwd}
            onChanged={() => {
              invalidateDir(cwd);
              void loadDir(cwd, true).then(setEntries);
            }}
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
