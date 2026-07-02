// Shared app actions, callable from hotkeys and the command palette.
// They read stores via getState() so they work outside React render.
import { confirmDialog } from "./stores/confirm";
import { useNotifications } from "./stores/notifications";
import { useWorkspaces } from "./stores/workspace";
import { useUi } from "./stores/ui";
import { pickFolders, worktreeCreate } from "./api";
import { useSettings, type ClaudePermission, type CodexMode } from "./stores/settings";
import { useProjectConfigs } from "./stores/projectConfig";
import { usePreview } from "./stores/preview";
import { maybeAutostart } from "./lib/startup";

export const WSL_CMD = "wsl";

// launch commands built from the user's per-provider settings (Settings → Providers).
// pass an override to launch with a specific mode (e.g. from the New Project wizard).
export function claudeCmd(mode?: ClaudePermission): string {
  const m = mode ?? useSettings.getState().claudePermission;
  if (m === "bypass") return "claude --dangerously-skip-permissions";
  if (m === "default") return "claude";
  return `claude --permission-mode ${m}`;
}
export function geminiCmd(yolo?: boolean): string {
  const y = yolo ?? useSettings.getState().geminiYolo;
  return y ? "gemini --yolo" : "gemini";
}
export function codexCmd(mode?: CodexMode): string {
  const m = mode ?? useSettings.getState().codexMode;
  if (m === "bypass") return "codex --dangerously-bypass-approvals-and-sandbox";
  // workspace-write sandbox + model-decides approval = the modern "full-auto" (which was removed)
  if (m === "auto") return "codex --sandbox workspace-write --ask-for-approval on-request";
  return "codex";
}
// opencode (sst) — open-source terminal agent, BYO-model. interactive TUI is just `opencode`;
// auth + model are configured in opencode itself (opencode auth / its config).
export function opencodeCmd(): string {
  return "opencode";
}
// grok (xAI's Grok Build CLI) — interactive TUI is just `grok`; it signs in with your grok login
// (browser token in ~/.grok) or an XAI_API_KEY, and picks its own default model.
export function grokCmd(): string {
  return "grok";
}

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
  // opening a terminal/agent in a project also kicks off its autostart services (folder-deduped)
  maybeAutostart(ws.id);
}

export const newClaude = () => launchInActive(claudeCmd());
export const newGemini = () => launchInActive(geminiCmd());
export const newCodex = () => launchInActive(codexCmd());
export const newOpencode = () => launchInActive(opencodeCmd());
export const newGrok = () => launchInActive(grokCmd());
export const newWsl = () => launchInActive(WSL_CMD);
export const newTerminal = () => launchInActive();

// Launch a Claude agent in its own isolated git worktree (branch hs/agent-N) so it can
// work in parallel without colliding with other agents in the same repo.
export async function newClaudeInWorktree() {
  const ws = activeWs();
  if (!ws || !ws.cwd) {
    useNotifications.getState().add({
      title: "New agent",
      body: "Open a project workspace (with a folder) first.",
    });
    return;
  }
  const name = `agent-${ws.sessions.length + 1}`;
  try {
    const path = await worktreeCreate(ws.cwd, name);
    useWorkspaces.getState().addSession(ws.id, claudeCmd(), path);
    // fire any of the project's actions flagged to run when a worktree is created, in the worktree
    for (const a of useProjectConfigs.getState().getConfig(ws.cwd).startup) {
      if (!a.runOnWorktree) continue;
      useWorkspaces.getState().addSession(ws.id, a.command || undefined, path);
      if (a.openPreview && a.previewUrl?.trim()) usePreview.getState().openUrl(a.previewUrl.trim());
    }
  } catch (e) {
    useNotifications.getState().add({ title: "Couldn't create worktree", body: String(e) });
  }
}

// close a pane; for a running AI session, confirm first so an agent mid-task
// isn't killed by a stray Ctrl+Shift+W or misclick. Plain terminals close instantly.
export async function closeSession(wsId: string, sessionId: string) {
  const ws = useWorkspaces.getState().workspaces.find((w) => w.id === wsId);
  const sess = ws?.sessions.find((s) => s.id === sessionId);
  if (!ws || !sess) return;
  const isAi = sess.provider === "claude" || sess.provider === "gemini" || sess.provider === "grok";
  // a pane whose command matches a configured startup task is a running service — confirm before killing
  const isService =
    !!ws.cwd &&
    useProjectConfigs
      .getState()
      .getConfig(ws.cwd)
      .startup.some((t) => (t.command ?? "") === (sess.command ?? ""));
  if ((isAi && sess.started) || isService) {
    const ok = await confirmDialog({
      title: isService ? "Stop service" : "Close pane",
      message: isService
        ? `"${sess.title || "This service"}" is running here. Stop it?`
        : `This ${sess.provider} session is still running. Close it anyway?`,
      confirmLabel: isService ? "Stop" : "Close",
      cancelLabel: "Cancel",
      danger: true,
      dontAskId: isService ? undefined : "close-running-pane",
    });
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
