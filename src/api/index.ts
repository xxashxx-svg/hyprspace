// Typed bridge over Tauri invoke()/Channel. Components import THIS, never invoke() directly.
import { invoke, Channel } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { CreatePtyOpts, FileChange, LicenseInfo, PtyControl, PtyHandlers } from "./types";

// A Raw send from Rust arrives as ArrayBuffer; a Json send arrives as the parsed control object.
type ChannelMsg = ArrayBuffer | PtyControl;

export async function createPty(opts: CreatePtyOpts, handlers: PtyHandlers): Promise<void> {
  const channel = new Channel<ChannelMsg>();
  channel.onmessage = (msg) => {
    if (msg instanceof ArrayBuffer) {
      handlers.onData(new Uint8Array(msg));
      return;
    }
    if (ArrayBuffer.isView(msg)) {
      const v = msg as ArrayBufferView;
      handlers.onData(new Uint8Array(v.buffer, v.byteOffset, v.byteLength));
      return;
    }
    let ctrl = msg as unknown;
    if (typeof ctrl === "string") {
      try {
        ctrl = JSON.parse(ctrl);
      } catch {
        return;
      }
    }
    handlers.onControl?.(ctrl as PtyControl);
  };

  await invoke("create_pty", {
    id: opts.id,
    cwd: opts.cwd,
    shell: opts.shell ?? null,
    args: opts.args ?? [],
    env: opts.env ?? {},
    cols: opts.cols,
    rows: opts.rows,
    onEvent: channel,
  });
}

export function writePty(id: string, data: Uint8Array): Promise<void> {
  return invoke("write_pty", { id, data: Array.from(data) });
}

export function resizePty(id: string, cols: number, rows: number): Promise<void> {
  return invoke("resize_pty", { id, cols, rows });
}

export function killPty(id: string): Promise<void> {
  return invoke("kill_pty", { id });
}

// ---- native chat: drive a persistent `claude` stream-json session (runs on the subscription) ----
// chatStart spawns the long-lived process and streams every event to onLine for its whole life;
// chatTurn feeds one user-message envelope to its stdin; chatStop kills it.
export function chatStart(
  id: string,
  cwd: string,
  args: string[],
  onLine: (line: string) => void,
): Promise<void> {
  const channel = new Channel<string>();
  channel.onmessage = (msg) => onLine(typeof msg === "string" ? msg : String(msg));
  return invoke("chat_start", { id, cwd, args, onEvent: channel });
}
export function chatTurn(id: string, message: string): Promise<void> {
  return invoke("chat_turn", { id, message });
}
export function chatStop(id: string): Promise<void> {
  return invoke("chat_stop", { id });
}

// ---- background services: run a command headless and stream its stdout+stderr as log lines ----
export function serviceStart(
  id: string,
  cwd: string,
  command: string,
  env: Record<string, string>,
  onLine: (line: string) => void,
): Promise<void> {
  const channel = new Channel<string>();
  channel.onmessage = (msg) => onLine(typeof msg === "string" ? msg : String(msg));
  return invoke("service_start", { id, cwd, command, env, onEvent: channel });
}
export function serviceStop(id: string): Promise<void> {
  return invoke("service_stop", { id });
}

// ---- loop agents: run ONE provider turn (args = full argv; prompt piped over stdin) ----
// streams stdout+stderr lines to onLine for the turn's life; the loop runner drives the loop around it.
export function agentStart(
  id: string,
  cwd: string,
  args: string[],
  env: Record<string, string>,
  prompt: string,
  onLine: (line: string) => void,
): Promise<void> {
  const channel = new Channel<string>();
  channel.onmessage = (msg) => onLine(typeof msg === "string" ? msg : String(msg));
  return invoke("agent_start", { id, cwd, args, env, prompt, onEvent: channel });
}
export function agentStop(id: string): Promise<void> {
  return invoke("agent_stop", { id });
}
export function getHomeDir(): Promise<string> {
  return invoke("get_home_dir");
}
// open a folder in the OS file manager
export function revealPath(path: string): Promise<void> {
  return invoke("reveal_path", { path });
}
// one directory level for the Files tree
export interface DirEntry {
  name: string;
  dir: boolean;
}
export function listDir(path: string): Promise<DirEntry[]> {
  return invoke("list_dir", { path });
}

export function saveState(name: string, data: string): Promise<void> {
  return invoke("save_state", { name, data });
}

export function loadState(name: string): Promise<string | null> {
  return invoke("load_state", { name });
}

export function backupState(name: string): Promise<void> {
  return invoke("backup_state", { name });
}

export function claudeHasHistory(cwd: string): Promise<boolean> {
  return invoke("claude_has_history", { cwd });
}

// how a returning claude pane should relaunch: "resume" | "continue" | "fresh"
export function claudeResumeMode(cwd: string, sessionId: string): Promise<string> {
  return invoke("claude_resume_mode", { cwd, sessionId });
}

// a folder's claude conversations as [sessionId, modifiedMs] — newest = the chat that's live now
export function claudeSessions(cwd: string): Promise<[string, number][]> {
  return invoke("claude_sessions", { cwd });
}

// loopback listener for the Google OAuth redirect; resolves with the target (carries ?code=...)
export function oauthListen(): Promise<string> {
  return invoke("oauth_listen");
}

export function shellName(): Promise<string> {
  return invoke("shell_name");
}

// null = not licensed yet (no key, or stored key no longer verifies)
export function licenseStatus(): Promise<LicenseInfo | null> {
  return invoke("license_status");
}

export function activateLicense(key: string): Promise<LicenseInfo> {
  return invoke("activate_license", { key });
}

// verify a subscription entitlement token offline (Ed25519, same key as licenses)
export interface Entitlement {
  uid: string;
  tier: string;
  mode: string;
  exp: number;
}
export function entitlementVerify(token: string): Promise<Entitlement> {
  return invoke("entitlement_verify", { token });
}

export function gitChanges(cwd: string): Promise<FileChange[]> {
  return invoke("git_changes", { cwd });
}

export function gitDiff(cwd: string, path: string): Promise<string> {
  return invoke("git_diff", { cwd, path });
}

export function detectRunCmd(cwd: string): Promise<string> {
  return invoke("detect_run_cmd", { cwd });
}

// git write ops for the topbar "Commit & push" menu + the Source Control panel
export function gitCommit(
  cwd: string,
  message: string,
  push: boolean,
  stageAll: boolean,
): Promise<string> {
  return invoke("git_commit", { cwd, message, push, stageAll });
}
export interface BranchInfo {
  branch: string;
  ahead: number;
  behind: number;
  upstream: boolean;
  is_repo: boolean;
}
export function gitBranchInfo(cwd: string): Promise<BranchInfo> {
  return invoke("git_branch_info", { cwd });
}
export function gitFileOp(cwd: string, op: "stage" | "unstage" | "discard", path: string): Promise<void> {
  return invoke("git_file_op", { cwd, op, path });
}
export function gitPush(cwd: string): Promise<string> {
  return invoke("git_push", { cwd });
}
export function gitCreatePr(cwd: string): Promise<string> {
  return invoke("git_create_pr", { cwd });
}
export function gitIsRepo(cwd: string): Promise<boolean> {
  return invoke("git_is_repo", { cwd });
}
export function gitInit(cwd: string): Promise<string> {
  return invoke("git_init", { cwd });
}
// create/reuse a project folder, optionally seeding README.md and .gitignore
export function createProjectDir(
  path: string,
  readme: string | null,
  gitignore: string | null,
): Promise<void> {
  return invoke("create_project_dir", { path, readme, gitignore });
}

// version + signed-in account/plan for an AI CLI ("claude" | "gemini" | "codex")
export interface ProviderStatus {
  id: string;
  installed: boolean;
  version: string | null;
  account: string | null;
  plan: string | null;
  detail: string | null;
}
export function providerStatus(id: string): Promise<ProviderStatus> {
  return invoke("provider_status", { id });
}

// MCP servers configured for Claude (~/.claude.json "mcpServers")
export interface McpEntry {
  name: string;
  config: Record<string, unknown>;
}
export function mcpList(): Promise<McpEntry[]> {
  return invoke("mcp_list");
}
export function mcpSet(name: string, config: unknown, prevName: string | null): Promise<void> {
  return invoke("mcp_set", { name, config, prevName });
}
export function mcpRemove(name: string): Promise<void> {
  return invoke("mcp_remove", { name });
}

// discovered Claude skills/commands (project + user scope) for the Skills panel
export interface SkillItem {
  name: string;
  command: string;
  description: string;
  body: string; // SKILL.md instructions (for inserting into non-Claude agents)
  scope: "project" | "user";
  kind: "skill" | "command";
}
export function listSkills(cwd: string): Promise<SkillItem[]> {
  return invoke("list_skills", { cwd });
}
// create/edit/delete a Claude skill (skills/<name>/SKILL.md) or command (commands/<name>.md)
export function skillRead(scope: string, cwd: string, name: string, kind: string): Promise<string> {
  return invoke("skill_read", { scope, cwd, name, kind });
}
export function skillWrite(
  scope: string,
  cwd: string,
  name: string,
  content: string,
  kind: string,
): Promise<void> {
  return invoke("skill_write", { scope, cwd, name, content, kind });
}
export function skillDelete(scope: string, cwd: string, name: string, kind: string): Promise<void> {
  return invoke("skill_delete", { scope, cwd, name, kind });
}

// ask the local `claude` CLI to name an open space from its terminal activity
export function aiNameSpace(context: string): Promise<string> {
  return invoke("ai_name_space", { context });
}

// create an isolated git worktree off the workspace repo; returns its path
export function worktreeCreate(cwd: string, name: string): Promise<string> {
  return invoke("worktree_create", { cwd, name });
}

export function worktreeRemove(cwd: string, path: string): Promise<void> {
  return invoke("worktree_remove", { cwd, path });
}

export async function pickFolder(): Promise<string | null> {
  const r = await open({ directory: true, multiple: false });
  return typeof r === "string" ? r : null;
}

export async function pickFile(): Promise<string | null> {
  const r = await open({ directory: false, multiple: false });
  return typeof r === "string" ? r : null;
}

export async function pickFolders(): Promise<string[]> {
  const r = await open({ directory: true, multiple: true });
  if (!r) return [];
  return Array.isArray(r) ? r : [r];
}
