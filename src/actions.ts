// Shared app actions, callable from hotkeys and the command palette.
// They read stores via getState() so they work outside React render.
import { ask } from "@tauri-apps/plugin-dialog";
import { useWorkspaces } from "./stores/workspace";
import { useUi } from "./stores/ui";
import { pickFolders } from "./api";

const CLAUDE_CMD = "claude --permission-mode auto";

function activeWs() {
  const { workspaces, activeId } = useWorkspaces.getState();
  return workspaces.find((w) => w.id === activeId) ?? null;
}

// open spaces launch into picked folder(s); projects launch in their own cwd
async function launchInActive(command?: string) {
  const ws = activeWs();
  if (!ws) return;
  if (ws.kind === "open") {
    const folders = await pickFolders();
    folders.forEach((f) => useWorkspaces.getState().addSession(ws.id, command, f));
  } else {
    useWorkspaces.getState().addSession(ws.id, command);
  }
}

export const newClaude = () => launchInActive(CLAUDE_CMD);
export const newTerminal = () => launchInActive();

// close a pane; for a running Claude session, confirm first so an agent mid-task
// isn't killed by a stray Ctrl+Shift+W or misclick. Plain terminals close instantly.
export async function closeSession(wsId: string, sessionId: string) {
  const ws = useWorkspaces.getState().workspaces.find((w) => w.id === wsId);
  const sess = ws?.sessions.find((s) => s.id === sessionId);
  if (!ws || !sess) return;
  if (sess.command?.includes("claude") && sess.started) {
    const ok = await ask("This Claude session is running. Close it?", {
      title: "Close pane",
      kind: "warning",
    }).catch(() => true);
    if (!ok) return;
  }
  useWorkspaces.getState().removeSession(wsId, sessionId);
}

export function closeFocused() {
  const ws = activeWs();
  const fid = useWorkspaces.getState().focusedSessionId;
  if (ws && fid) void closeSession(ws.id, fid);
}

export function toggleMaxFocused() {
  const ws = activeWs();
  const fid = useWorkspaces.getState().focusedSessionId;
  if (ws && fid && ws.sessions.some((s) => s.id === fid)) useUi.getState().toggleMaximized(fid);
}

export function switchSpaceByIndex(i: number) {
  const w = useWorkspaces.getState().workspaces[i];
  if (w) useWorkspaces.getState().setActive(w.id);
}

export function cycleSpace(dir: 1 | -1) {
  const { workspaces, activeId } = useWorkspaces.getState();
  if (!workspaces.length) return;
  const idx = workspaces.findIndex((w) => w.id === activeId);
  const next = (idx + dir + workspaces.length) % workspaces.length;
  useWorkspaces.getState().setActive(workspaces[next].id);
}

// cycle keyboard focus through the active space's panes
export function cyclePane(dir: 1 | -1) {
  const ws = activeWs();
  if (!ws || ws.sessions.length === 0) return;
  const fid = useWorkspaces.getState().focusedSessionId;
  const idx = ws.sessions.findIndex((s) => s.id === fid);
  const start = idx < 0 ? 0 : idx;
  const next = (start + dir + ws.sessions.length) % ws.sessions.length;
  useWorkspaces.getState().setFocused(ws.sessions[next].id);
}
