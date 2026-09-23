// Shared app actions, callable from hotkeys and the command palette.
// They read stores via getState() so they work outside React render.
import { confirmDialog } from "./stores/confirm";
import { isFileDirty, markDiscarded } from "./lib/dirtyFiles";
import { useWorkspaces } from "./stores/workspace";
import { useUi } from "./stores/ui";
import { pickFolder, pickFolders } from "./api";
import { useSettings, type ClaudePermission, type CodexMode } from "./stores/settings";
import { modelFlags, type ProviderId } from "./lib/models";

export const WSL_CMD = "wsl";

// Launch commands, built from the per-agent settings (permission mode, model, effort).
// Pass overrides to launch with a specific choice, for example from the composer.
export interface LaunchChoice {
  model?: string;
  effort?: string;
}

// the saved model / effort for a provider, unless the caller picked something else
function choice(provider: ProviderId, c?: LaunchChoice): { model: string; effort: string } {
  const s = useSettings.getState();
  return {
    model: c?.model ?? s.agentModel[provider] ?? "",
    effort: c?.effort ?? s.agentEffort[provider] ?? "",
  };
}

function withModel(base: string, provider: ProviderId, c?: LaunchChoice): string {
  const { model, effort } = choice(provider, c);
  return [base, ...modelFlags(provider, model, effort)].join(" ");
}

export function claudeCmd(mode?: ClaudePermission, c?: LaunchChoice): string {
  const m = mode ?? useSettings.getState().claudePermission;
  const base =
    m === "bypass" ? "claude --dangerously-skip-permissions" : m === "default" ? "claude" : `claude --permission-mode ${m}`;
  return withModel(base, "claude", c);
}
export function geminiCmd(yolo?: boolean, c?: LaunchChoice): string {
  const y = yolo ?? useSettings.getState().geminiYolo;
  return withModel(y ? "gemini --yolo" : "gemini", "gemini", c);
}
export function codexCmd(mode?: CodexMode, c?: LaunchChoice): string {
  const m = mode ?? useSettings.getState().codexMode;
  const base =
    m === "bypass"
      ? "codex --dangerously-bypass-approvals-and-sandbox"
      : m === "auto"
        ? "codex --sandbox workspace-write --ask-for-approval on-request"
        : "codex";
  return withModel(base, "codex", c);
}
export function opencodeCmd(c?: LaunchChoice): string {
  return withModel("opencode", "opencode", c);
}
export function grokCmd(c?: LaunchChoice): string {
  return withModel("grok", "grok", c);
}

/** The launch command that reopens a saved conversation. Claude and Codex resume by id. */
export function resumeCmd(provider: ProviderId, id: string, c?: LaunchChoice): string | undefined {
  if (provider === "claude") return claudeCmd(undefined, c).replace(/^claude\b/, `claude --resume ${id}`);
  if (provider === "codex") return codexCmd(undefined, c).replace(/^codex\b/, `codex resume ${id}`);
  return undefined;
}

/** The launch command for any provider, with an optional model / effort override. */
export function commandFor(provider: ProviderId, c?: LaunchChoice): string | undefined {
  switch (provider) {
    case "claude":
      return claudeCmd(undefined, c);
    case "codex":
      return codexCmd(undefined, c);
    case "gemini":
      return geminiCmd(undefined, c);
    case "opencode":
      return opencodeCmd(c);
    case "grok":
      return grokCmd(c);
    case "wsl":
      return WSL_CMD;
    default:
      return undefined;
  }
}

function activeWs() {
  const { workspaces, activeId } = useWorkspaces.getState();
  return workspaces.find((w) => w.id === activeId) ?? null;
}

// open spaces launch into picked folder(s); projects launch in their own cwd
export async function launchInActive(command?: string) {
  const ws = activeWs();
  if (!ws) return;
  if (ws.kind === "open") {
    const folders = await pickFolders();
    folders.forEach((f) => useWorkspaces.getState().addSession(ws.id, command, f));
  } else {
    useWorkspaces.getState().addSession(ws.id, command);
  }
}

/** A new composer pane in the active space. With no space open, the home page is the composer. */
export function newSession() {
  const st = useWorkspaces.getState();
  const ws = st.workspaces.find((w) => w.id === st.activeId);
  if (!ws) {
    useUi.getState().goHome();
    return;
  }
  st.addDraft(ws.id);
  useUi.getState().goSpace();
}

export const newTerminal = () => launchInActive();

/** Pick a folder and open it as a new space. */
export async function openFolderAsSpace() {
  const folder = await pickFolder();
  if (!folder) return;
  useWorkspaces.getState().addWorkspace(folder.split(/[\\/]/).filter(Boolean).pop() || "Project", folder);
  if (useUi.getState().view !== "home") useUi.getState().goSpace();
}

// close a pane; for a running AI session, confirm first so an agent mid-task
// isn't killed by a stray Ctrl+Shift+W or misclick. Plain terminals close instantly.
export async function closeSession(wsId: string, sessionId: string) {
  const ws = useWorkspaces.getState().workspaces.find((w) => w.id === wsId);
  const sess = ws?.sessions.find((s) => s.id === sessionId);
  if (!ws || !sess) return;
  // an editor tab with unsaved edits: ask first. the editor flushes on unmount so nothing is lost
  // either way, but writing to disk because you clicked × is a surprise — make it a choice.
  if (sess.file && isFileDirty(sess.file)) {
    const ans = await confirmDialog({
      title: "Unsaved changes",
      message: `"${sess.title || sess.file}" has unsaved changes.`,
      confirmLabel: "Save & close",
      cancelLabel: "Cancel",
      altLabel: "Discard",
    });
    if (ans === false) return;
    // tell the editor not to flush on the way out — otherwise unmount would re-save the buffer
    if (ans === "alt") markDiscarded(sess.file);
  }
  const isAi = sess.provider === "claude" || sess.provider === "gemini" || sess.provider === "grok";
  if (isAi && sess.started) {
    const ok = await confirmDialog({
      title: "Close pane",
      message: `This ${sess.provider} session is still running. Close it anyway?`,
      confirmLabel: "Close",
      cancelLabel: "Cancel",
      danger: true,
      dontAskId: "close-running-pane",
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
  if (ws && fid && ws.sessions.some((s) => s.id === fid)) useUi.getState().toggleMaximized(ws.id, fid);
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
