// Control messages sent over the PTY channel as JSON (raw bytes come as ArrayBuffer instead).
export interface PtyExit {
  type: "exit";
  code: number;
}
export interface PtyError {
  type: "error";
  message: string;
}
export type PtyControl = PtyExit | PtyError;

export interface CreatePtyOpts {
  id: string;
  cwd: string;
  shell?: string;
  args?: string[];
  env?: Record<string, string>;
  cols: number;
  rows: number;
}

export interface PtyHandlers {
  onData: (bytes: Uint8Array) => void;
  onControl?: (c: PtyControl) => void;
}

export interface LicenseInfo {
  name: string;
  issued: string;
}

export interface FileChange {
  path: string;
  status: string; // git porcelain code: M, A, D, R, ??, …
  added: number;
  removed: number;
}
