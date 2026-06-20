import { useEffect, useState } from "react";
import { useProjectConfigs, folderKey, type StartupTask } from "../stores/projectConfig";
import { useWorkspaces, type Session } from "../stores/workspace";
import { useUi } from "../stores/ui";
import { useServices, serviceId } from "../stores/services";
import { launchTask, startServices, taskFromFile } from "../lib/startup";
import { closeSession } from "../actions";
import { pickFile } from "../api";
import { Play, Plus, X, Trash2, Upload, Square, ScrollText } from "lucide-react";

const uid = () => crypto.randomUUID();
const EMPTY_SESSIONS: Session[] = [];

// a task's live run: a background service or a pane; `running` false = stopped but logs still around
type Run = { bg: boolean; key: string; running: boolean };

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

  const apply = (patch: Partial<{ startup: StartupTask[]; env: Record<string, string>; defaultShell?: string }>) =>
    useProjectConfigs.getState().setConfig(folder, patch);
  const setTasks = (next: StartupTask[]) => apply({ startup: next });
  const updateTask = (id: string, patch: Partial<StartupTask>) =>
    setTasks(startup.map((t) => (t.id === id ? { ...t, ...patch } : t)));

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

  // run-state: a background service (headless, logs captured) or a pane. `running` is the live state;
  // a background service that has stopped but still has logs is reported with running=false so its
  // output stays reachable.
  const sessions = useWorkspaces((s) => s.workspaces.find((w) => w.id === wsId)?.sessions ?? EMPTY_SESSIONS);
  const bgRunning = useServices((s) => s.running);
  const bgKnown = useServices((s) => s.known);
  const runState = (t: StartupTask): Run | null => {
    if (t.background) {
      const sid = serviceId(t.id);
      if (bgRunning[sid]) return { bg: true, key: sid, running: true };
      if (bgKnown[sid]) return { bg: true, key: sid, running: false }; // stopped — logs still viewable
      return null;
    }
    const sess = wsId ? sessions.find((s) => (s.command ?? "") === (t.command ?? "")) : undefined;
    return sess ? { bg: false, key: sess.id, running: true } : null;
  };
  const viewLogs = (run: Run, t: StartupTask) => {
    if (run.bg) {
      useUi.getState().openServiceLogs({ id: run.key, name: t.name || "service" });
      return;
    }
    if (!wsId) return;
    useWorkspaces.getState().setActive(wsId);
    useWorkspaces.getState().setFocused(run.key);
    useUi.getState().closeServices();
    useUi.getState().goSpace();
  };
  const stopRun = (run: Run) => {
    if (run.bg) useServices.getState().stop(run.key);
    else if (wsId) void closeSession(wsId, run.key);
  };

  return (
    <div className="svc">
      <div className="svc-head">
        <span className="svc-title" title={folder}>
          {name || folder}
        </span>
        {wsId && startup.length > 0 && (
          <button className="svc-startall" onClick={() => startServices(wsId, { force: true })}>
            <Play size={11} /> Start all
          </button>
        )}
      </div>

      <div className="svc-section">
        <div className="svc-label">Startup tasks</div>
        <button className="svc-drop" data-folder={folder} onClick={() => void browseFile()}>
          <Upload size={15} />
          <span>
            Drop a <b>.bat</b> / script / <b>.exe</b> here, or click to browse
          </span>
        </button>
        {startup.map((t) => {
          const run = runState(t);
          return (
            <div className={`svc-task${run?.running ? " running" : ""}`} key={t.id}>
              <div className="svc-task-row">
                {run?.running && <span className="svc-dot on" title="Running" />}
                <input
                  className="svc-in svc-name"
                  placeholder="Name"
                  value={t.name}
                  onChange={(e) => updateTask(t.id, { name: e.target.value })}
                />
                {wsId && (
                  <>
                    {run && (run.bg || run.running) && (
                      <button className="svc-run on" title="View logs" onClick={() => viewLogs(run, t)}>
                        <ScrollText size={13} />
                      </button>
                    )}
                    {run?.running ? (
                      <button className="svc-stop" title="Stop" onClick={() => stopRun(run)}>
                        <Square size={13} />
                      </button>
                    ) : (
                      <button className="svc-run" title="Run" onClick={() => launchTask(wsId, t)}>
                        <Play size={13} />
                      </button>
                    )}
                  </>
                )}
                <button
                  className="svc-del"
                  title="Remove"
                  onClick={() => setTasks(startup.filter((x) => x.id !== t.id))}
                >
                  <Trash2 size={13} />
                </button>
              </div>
              <input
                className="svc-in svc-cmd"
                placeholder="Command (e.g. npm run dev)"
                value={t.command}
                onChange={(e) => updateTask(t.id, { command: e.target.value })}
              />
              <input
                className="svc-in svc-folder"
                placeholder="Subfolder (optional)"
                value={t.folder ?? ""}
                onChange={(e) => updateTask(t.id, { folder: e.target.value || undefined })}
              />
              <div className="svc-task-row svc-flags">
                <label className="svc-auto">
                  <input
                    type="checkbox"
                    checked={t.autostart}
                    onChange={(e) => updateTask(t.id, { autostart: e.target.checked })}
                  />
                  Autostart
                </label>
                <label className="svc-auto">
                  <input
                    type="checkbox"
                    checked={!!t.background}
                    onChange={(e) => updateTask(t.id, { background: e.target.checked })}
                  />
                  Background
                </label>
              </div>
            </div>
          );
        })}
        <button
          className="svc-add"
          onClick={() => setTasks([...startup, { id: uid(), name: "", command: "", autostart: false }])}
        >
          <Plus size={13} /> Add command
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
        Autostart tasks launch when you open a project — or a terminal — in this folder. <b>Background</b>{" "}
        tasks run headless with no pane; click the log icon to watch their output. Otherwise a task gets
        its own terminal pane.
      </div>
    </div>
  );
}
