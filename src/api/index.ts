// Typed bridge over Tauri invoke()/Channel. Components import THIS, never invoke() directly.
import { invoke, Channel } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { CreatePtyOpts, LicenseInfo, PtyControl, PtyHandlers } from "./types";

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

// null = not licensed yet (no key, or stored key no longer verifies)
export function licenseStatus(): Promise<LicenseInfo | null> {
  return invoke("license_status");
}

export function activateLicense(key: string): Promise<LicenseInfo> {
  return invoke("activate_license", { key });
}

export async function pickFolder(): Promise<string | null> {
  const r = await open({ directory: true, multiple: false });
  return typeof r === "string" ? r : null;
}

export async function pickFolders(): Promise<string[]> {
  const r = await open({ directory: true, multiple: true });
  if (!r) return [];
  return Array.isArray(r) ? r : [r];
}
