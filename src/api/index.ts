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
    autoRespond: opts.autoRespond ?? false,
  });
}

// input crosses IPC as base64 — a JSON number array was ~4-5 bytes per input byte, which made
// big pastes build multi-MB JSON strings on the UI thread
function toBase64(data: Uint8Array): string {
  let s = "";
  const CHUNK = 0x8000; // fromCharCode arg-count limit
  for (let i = 0; i < data.length; i += CHUNK) {
    s += String.fromCharCode(...data.subarray(i, i + CHUNK));
  }
  return btoa(s);
}

export function writePty(id: string, data: Uint8Array): Promise<void> {
  return invoke("write_pty", { id, data: toBase64(data) });
}

export function resizePty(id: string, cols: number, rows: number): Promise<void> {
  return invoke("resize_pty", { id, cols, rows });
}

// xterm flow control: pause stops the backend PTY reader (kernel buffer then backpressures the
// child); resume when xterm's write buffer has drained
export function pausePty(id: string): Promise<void> {
  return invoke("pause_pty", { id });
}
export function resumePty(id: string): Promise<void> {
  return invoke("resume_pty", { id });
}

export function killPty(id: string): Promise<void> {
  return invoke("kill_pty", { id });
}

// ---- background services: run a command headless and stream its stdout+stderr as log lines ----
// the backend batches lines (~30ms) and sends them joined with '\n' so a chatty dev server
// doesn't cost one IPC hop per line — onLines gets the whole batch, one store write per batch
export function serviceStart(
  id: string,
  cwd: string,
  command: string,
  env: Record<string, string>,
  onLines: (lines: string[]) => void,
): Promise<void> {
  const channel = new Channel<string>();
  channel.onmessage = (msg) => onLines((typeof msg === "string" ? msg : String(msg)).split("\n"));
  return invoke("service_start", { id, cwd, command, env, onEvent: channel });
}
export function serviceStop(id: string): Promise<void> {
  return invoke("service_stop", { id });
}

// ---- loop agents: run ONE provider turn (args = full argv; prompt piped over stdin) ----
// streams stdout+stderr lines to onLine for the turn's life; the loop runner drives the loop around it.
// `secrets` maps an env-var name → a keychain secret name (e.g. { ANTHROPIC_API_KEY: "anthropic" }).
// Rust reads each from the OS keychain and injects it into the child env — the value never enters JS.
export function agentStart(
  id: string,
  cwd: string,
  args: string[],
  env: Record<string, string>,
  secrets: Record<string, string>,
  prompt: string,
  onLine: (line: string) => void,
): Promise<void> {
  const channel = new Channel<string>();
  channel.onmessage = (msg) => onLine(typeof msg === "string" ? msg : String(msg));
  return invoke("agent_start", { id, cwd, args, env, secrets, prompt, onEvent: channel });
}
export function agentStop(id: string): Promise<void> {
  return invoke("agent_stop", { id });
}

// OS keychain — store/check/clear a named secret (write-only from the UI; the value never comes back)
export function secretSet(name: string, value: string): Promise<void> {
  return invoke("secret_set", { name, value });
}
export function secretHas(name: string): Promise<boolean> {
  return invoke("secret_has", { name });
}
export function secretClear(name: string): Promise<void> {
  return invoke("secret_clear", { name });
}
export function getHomeDir(): Promise<string> {
  return invoke("get_home_dir");
}
// Alt+V image paste: read the clipboard image ourselves and write a temp PNG (dodges the flaky
// first clipboard read the CLIs hit). Returns the file path, or null when there's no image.
export function clipboardImageToTemp(): Promise<string | null> {
  return invoke("clipboard_image_to_temp");
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
// editor file IO — read_file rejects >2MB and binary files; write_file saves the buffer
export function readFile(path: string): Promise<string> {
  return invoke("read_file", { path });
}
export function writeFile(path: string, content: string): Promise<void> {
  return invoke("write_file", { path, content });
}
// read an image file as a data URL for the in-app image viewer (caps at 25MB)
export function readImageFile(path: string): Promise<string> {
  return invoke("read_image_file", { path });
}
// does a path exist on disk? the terminal image linkifier only lights up real files (Orca-style)
export function pathExists(path: string): Promise<boolean> {
  return invoke("path_exists", { path });
}
// resolve Claude Code's `[Image #N]` marker → the cached image file for this pane's session, or null
export function claudeImagePath(cwd: string, n: number): Promise<string | null> {
  return invoke("claude_image_path", { cwd, n });
}
// Files-panel ops: create-file / create-dir / rename (to = new full path) / delete
export function fileOp(op: "create-file" | "create-dir" | "rename" | "delete", path: string, to?: string): Promise<void> {
  return invoke("file_op", { op, path, to });
}
// recursive filename search under root; returns root-relative paths (files only, capped)
export function findFiles(root: string, query: string): Promise<string[]> {
  return invoke("find_files", { root, query });
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

// run a one-shot shell command in cwd; resolves with its exit code (-1 if it can't start).
// the Loops "until check passes" guard uses this (exit 0 = the goal is met → stop).
export function runCheck(cwd: string, command: string): Promise<number> {
  return invoke("run_check", { cwd, command });
}

// drop an automation run's temp hook/notify files (settings, markers) once it ends
export function cleanupHookRun(runId: string): Promise<void> {
  return invoke("cleanup_hook_run", { runId });
}
// settings file with a Notification hook for an interactive-terminal loop (pings when Claude needs you)
export function prepareNotifySettings(runId: string): Promise<{ settings: string; marker: string; done: string }> {
  return invoke("prepare_notify_settings", { runId });
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
export interface PrDefaults {
  head: string;
  base: string;
  title: string;
  body: string;
  branches: string[];
  pushed: boolean;
  onDefault: boolean;
}
export function gitPrDefaults(cwd: string): Promise<PrDefaults> {
  return invoke("git_pr_defaults", { cwd });
}
export function gitCreatePr(opts: {
  cwd: string;
  title: string;
  body: string;
  base: string;
  draft: boolean;
  push: boolean;
}): Promise<string> {
  return invoke("git_create_pr", opts);
}
export function gitIsRepo(cwd: string): Promise<boolean> {
  return invoke("git_is_repo", { cwd });
}
export function gitInit(cwd: string): Promise<string> {
  return invoke("git_init", { cwd });
}
export function gitInitRepo(opts: {
  cwd: string;
  name: string;
  branch: string;
  gitignore: string;
  readme: boolean;
  commit: boolean;
  commitMsg: string;
  github: boolean;
  private: boolean;
  description: string;
}): Promise<string> {
  return invoke("git_init_repo", opts);
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

// per-provider usage, read display-only from the CLIs' own local files (never an API call)
export interface UsageWindow {
  usedPercent: number;
  windowMinutes: number;
  resetsAt: number; // unix seconds
}
export interface UsageDay {
  date: string;
  value: number;
}
export interface UsageModel {
  model: string;
  inputTokens: number;
  outputTokens: number;
  cacheTokens: number;
  totalTokens: number;
}
export interface ProviderUsage {
  id: string;
  label: string;
  signedIn: boolean;
  account: string | null;
  plan: string | null;
  tier: string | null;
  sessions: number;
  messages: number;
  toolCalls: number;
  activeDays: number;
  inputTokens: number;
  outputTokens: number;
  cacheTokens: number;
  totalTokens: number;
  tokensWindow: string | null;
  primary: UsageWindow | null;
  secondary: UsageWindow | null;
  daily: UsageDay[];
  dailyUnit: string | null; // "tokens" | "msgs" | "sessions"
  models: UsageModel[];
  note: string | null;
}
// single provider — lets the usage panel stream cards in as each scan finishes
export function providerUsageOne(id: string): Promise<ProviderUsage | null> {
  return invoke("provider_usage_one", { id });
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
