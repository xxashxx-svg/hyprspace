// Launching a folder's configured actions (its dev server, db, watchers, agents, …).
// Config is keyed by folder, so opening the same folder in several projects shares one config —
// and we dedup by command across every workspace at that folder, so a server never runs twice.
import { useWorkspaces } from "../stores/workspace";
import { useProjectConfigs, folderKey, type Action } from "../stores/projectConfig";
import { usePreview } from "../stores/preview";
import { isWindows } from "../platform";

// turn a dropped/picked file (a .bat / .ps1 / .sh / .exe / …) into an action that runs it
export function taskFromFile(path: string): Action {
  const name = path.split(/[\\/]/).pop() || "Task";
  const ext = name.split(".").pop()?.toLowerCase();
  let command: string;
  if (isWindows) {
    // PowerShell (the default shell) runs .bat/.cmd/.exe by path via the call operator; .ps1 needs
    // an explicit policy bypass to be safe
    command =
      ext === "ps1" ? `powershell -ExecutionPolicy Bypass -File "${path}"` : `& "${path}"`;
  } else {
    command = ext === "sh" ? `bash "${path}"` : `"${path}"`;
  }
  return { id: crypto.randomUUID(), name, command };
}

// folders we've already autostarted this app session — so opening the same folder again (or a
// second project at it) doesn't relaunch
const autostartedFolders = new Set<string>();

function taskCwd(wsCwd: string, folder?: string): string {
  if (!folder) return wsCwd;
  if (!wsCwd) return folder;
  const sep = wsCwd.includes("\\") ? "\\" : "/";
  return `${wsCwd}${sep}${folder}`;
}

// launch one action as a terminal pane
export function launchTask(wsId: string, task: Action) {
  const ws = useWorkspaces.getState().workspaces.find((w) => w.id === wsId);
  if (!ws) return;
  useWorkspaces.getState().addSession(wsId, task.command || undefined, taskCwd(ws.cwd, task.folder));
}

// run an action on demand: launch it, and open its preview URL in the embedded preview if asked
export function runAction(wsId: string, action: Action) {
  launchTask(wsId, action);
  if (action.openPreview && action.previewUrl?.trim()) {
    usePreview.getState().openUrl(action.previewUrl.trim());
  }
}

// run the folder's actions flagged to auto-run when a worktree is created for it
export function runWorktreeActions(wsId: string) {
  const ws = useWorkspaces.getState().workspaces.find((w) => w.id === wsId);
  if (!ws || !ws.cwd) return;
  const cfg = useProjectConfigs.getState().getConfig(ws.cwd);
  for (const a of cfg.startup) if (a.runOnWorktree) runAction(wsId, a);
}

// run a folder's auto-run actions in a workspace. dedups by command across ALL workspaces at the
// same folder, so the same server can't be started twice. force = run all actions, not just auto ones.
export function runFolderActions(wsId: string, opts?: { force?: boolean }) {
  const ws = useWorkspaces.getState().workspaces.find((w) => w.id === wsId);
  if (!ws || ws.kind === "open" || !ws.cwd) return;
  const key = folderKey(ws.cwd);
  const cfg = useProjectConfigs.getState().getConfig(ws.cwd);
  // every command currently running in any workspace at this folder
  const running = new Set(
    useWorkspaces
      .getState()
      .workspaces.filter((w) => folderKey(w.cwd) === key)
      .flatMap((w) => w.sessions.map((s) => s.command ?? "")),
  );
  for (const t of cfg.startup) {
    if (!opts?.force && !t.runOnOpen) continue;
    if (running.has(t.command ?? "")) continue; // pane already running this command
    launchTask(wsId, t);
    running.add(t.command ?? "");
  }
}

// autostart a folder's services the first time it's opened this session (once per folder, no matter
// how many projects point at it)
export function maybeAutostart(wsId: string) {
  const ws = useWorkspaces.getState().workspaces.find((w) => w.id === wsId);
  if (!ws || ws.kind === "open" || !ws.cwd) return;
  const key = folderKey(ws.cwd);
  if (autostartedFolders.has(key)) return;
  const cfg = useProjectConfigs.getState().getConfig(ws.cwd);
  if (!cfg.startup.some((t) => t.runOnOpen)) return;
  autostartedFolders.add(key);
  runFolderActions(wsId);
}
