import { useEffect, useState } from "react";
import { useProjectConfigs, folderKey, type Action } from "../stores/projectConfig";
import { useActionEditor } from "../stores/actionEditor";
import { useWorkspaces, type Session } from "../stores/workspace";
import { useUi } from "../stores/ui";
import { runAction, runFolderActions, taskFromFile } from "../lib/startup";
import { closeSession } from "../actions";
import { pickFile } from "../api";
import { Play, Plus, X, Trash2, Upload, Square, ScrollText, Keyboard } from "lucide-react";

const uid = () => crypto.randomUUID();
const EMPTY_SESSIONS: Session[] = [];

// a task's live run — the pane it's running in
type Run = { key: string };

// Editor for one folder's project config. `wsId` (when a project at this folder is open) enables
// the Start buttons; without it (e.g. in Settings) it's config-only.
export function ServicesEditor({ folder, name, wsId }: { folder: string; name?: string; wsId?: string }) {
  const cfg = useProjectConfigs((s) => s.configs[folderKey(folder)]);
  const startup = cfg?.startup ?? [];
  const env = cfg?.env ?? {};

  // env edited as a local row list so renaming a key mid-type doesn't fight the object
  const [envRows, setEnvRows] = useState<{ id: string; k: string; v: string }[]>([]);
  useEffect(() => {
    setEnvRows(Object.entries(env).map(([k, v]) => ({ id: uid(), k, v })));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [folder]); // reload when the folder changes

  const apply = (patch: Partial<{ startup: Action[]; env: Record<string, string>; defaultShell?: string }>) =>
    useProjectConfigs.getState().setConfig(folder, patch);
  const setTasks = (next: Action[]) => apply({ startup: next });
  const editAction = (action?: Action) =>
    useActionEditor.getState().openEditor(folder, { wsId, action });

  const commitEnv = (rows: { id: string; k: string; v: string }[]) => {
    setEnvRows(rows);
    const obj: Record<string, string> = {};
    for (const r of rows) if (r.k.trim()) obj[r.k.trim()] = r.v;
    apply({ env: obj });
  };

  const browseFile = async () => {
    const f = await pickFile();
    if (f) setTasks([...startup, taskFromFile(f)]);
  };

  // run-state: the pane this action is currently running in, if any
  const sessions = useWorkspaces((s) => s.workspaces.find((w) => w.id === wsId)?.sessions ?? EMPTY_SESSIONS);
  const runState = (t: Action): Run | null => {
    const sess = wsId ? sessions.find((s) => (s.command ?? "") === (t.command ?? "")) : undefined;
    return sess ? { key: sess.id } : null;
  };
  const viewLogs = (run: Run) => {
    if (!wsId) return;
    useWorkspaces.getState().setActive(wsId);
    useWorkspaces.getState().setFocused(run.key);
    useUi.getState().goSpace();
  };
  const stopRun = (run: Run) => {
    if (wsId) void closeSession(wsId, run.key);
  };

  return (
    <div className="svc">
      <div className="svc-head">
        <span className="svc-title" title={folder}>
          {name || folder}
        </span>
        {wsId && startup.length > 0 && (
          <button className="svc-startall" onClick={() => runFolderActions(wsId, { force: true })}>
            <Play size={11} /> Start all
          </button>
        )}
      </div>

      <div className="svc-section">
        <div className="svc-label">Actions</div>
        <button className="svc-drop" data-folder={folder} onClick={() => void browseFile()}>
          <Upload size={15} />
          <span>
            Drop a <b>.bat</b> / script / <b>.exe</b> to add it, or click to browse
          </span>
        </button>
        {startup.map((a) => {
          const run = runState(a);
          return (
            <div className={`act-card${run ? " running" : ""}`} key={a.id}>
              <button className="act-card-main" title="Edit action" onClick={() => editAction(a)}>
                <span className={`act-card-dot${run ? " on" : ""}`} />
                <span className="act-card-text">
                  <span className="act-card-name">{a.name || "Untitled action"}</span>
                  <span className="act-card-cmd">{a.command || "(no command)"}</span>
                </span>
                {a.keybinding && (
                  <span className="act-card-key" title="Keybinding">
                    <Keyboard size={11} /> {a.keybinding}
                  </span>
                )}
              </button>
              <div className="act-card-btns">
                {wsId && run && (
                  <button className="svc-run on" title="Go to its pane" onClick={() => viewLogs(run)}>
                    <ScrollText size={13} />
                  </button>
                )}
                {wsId &&
                  (run ? (
                    <button className="svc-stop" title="Stop" onClick={() => stopRun(run)}>
                      <Square size={13} />
                    </button>
                  ) : (
                    <button className="svc-run" title="Run" onClick={() => runAction(wsId, a)}>
                      <Play size={13} />
                    </button>
                  ))}
                <button
                  className="svc-del"
                  title="Remove"
                  onClick={() => setTasks(startup.filter((x) => x.id !== a.id))}
                >
                  <Trash2 size={13} />
                </button>
              </div>
            </div>
          );
        })}
        <button className="svc-add" onClick={() => editAction()}>
          <Plus size={13} /> Add action
        </button>
      </div>

      <div className="svc-section">
        <div className="svc-label">Environment variables</div>
        {envRows.map((r) => (
          <div className="svc-env-row" key={r.id}>
            <input
              className="svc-in"
              placeholder="KEY"
              value={r.k}
              onChange={(e) => commitEnv(envRows.map((x) => (x.id === r.id ? { ...x, k: e.target.value } : x)))}
            />
            <span className="svc-eq">=</span>
            <input
              className="svc-in"
              placeholder="value"
              value={r.v}
              onChange={(e) => commitEnv(envRows.map((x) => (x.id === r.id ? { ...x, v: e.target.value } : x)))}
            />
            <button className="svc-del" onClick={() => commitEnv(envRows.filter((x) => x.id !== r.id))}>
              <X size={13} />
            </button>
          </div>
        ))}
        <button className="svc-add" onClick={() => setEnvRows([...envRows, { id: uid(), k: "", v: "" }])}>
          <Plus size={13} /> Add variable
        </button>
      </div>

      <div className="svc-section">
        <div className="svc-label">Default shell</div>
        <input
          className="svc-in"
          placeholder="System default (e.g. bash, pwsh, cmd)"
          value={cfg?.defaultShell ?? ""}
          onChange={(e) => apply({ defaultShell: e.target.value || undefined })}
        />
      </div>

      <div className="svc-hint">
        Actions are commands you run on demand — from the top-bar <b>Actions</b> menu, the command
        palette, or a keybinding. Turn on <b>Run on open / worktree</b> in an action to auto-run it.
        Each one opens in its own terminal pane.
      </div>
    </div>
  );
}
