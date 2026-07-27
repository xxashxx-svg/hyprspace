// Connection settings + the state mirror the desktop pushes. The mirror is read-only here: anything
// that changes it goes back over the wire as a request and returns as a fresh snapshot.
import { create } from "zustand";
import AsyncStorage from "@react-native-async-storage/async-storage";
import { Platform } from "react-native";

// ---- the snapshot shape, mirroring src/mobileBridge.ts on the desktop ----

export interface Pane {
  id: string;
  title: string;
  provider: string;
  cwd: string;
  started: boolean;
  state: "working" | "waiting" | "done" | "idle";
  activity: string | null;
  subs: number;
}

export interface Space {
  id: string;
  name: string;
  kind: "project" | "open";
  cwd: string;
  color: string;
  activated: boolean;
  panes: Pane[];
}

export interface Automation {
  id: string;
  name: string;
  mode: string;
  enabled: boolean;
  folder: string;
  status: string;
  lastRunAt: number | null;
  nextRunAt: number | null;
  lastResult: string | null;
  wsId: string | null;
  paneId: string | null;
}

export interface UsageWindow {
  pct: number;
  resetsAt?: number;
}

export interface Usage {
  five?: UsageWindow;
  others: { key: string; label: string; win: UsageWindow }[];
  models: string[];
  at: number;
  stale: boolean;
}

export interface Snap {
  at: number;
  activeId: string | null;
  focusedId: string | null;
  spaces: Space[];
  automations: Automation[];
  usage: Usage | null;
}

export type Status = "unpaired" | "offline" | "connecting" | "online" | "retrying" | "failed";

interface ConnState {
  // paired desktop
  host: string;
  port: number;
  token: string;
  deviceName: string;
  loaded: boolean;

  status: Status;
  error: string | null;
  retryIn: number;
  desktopHost: string;
  desktopVersion: string;
  snap: Snap | null;

  load: () => Promise<void>;
  pair: (p: { host: string; port: number; token: string }) => Promise<void>;
  forget: () => Promise<void>;
  setDeviceName: (n: string) => Promise<void>;

  connecting: () => void;
  online: (d: { host: string; version: string }) => void;
  offline: () => void;
  retrying: (secs: number) => void;
  failed: (msg: string) => void;
  setSnap: (s: Snap | null) => void;
}

const KEY = "hyprspace.conn";

function defaultName() {
  return Platform.OS === "android" ? "Android phone" : "Phone";
}

export const useConn = create<ConnState>()((set, get) => ({
  host: "",
  port: 6768,
  token: "",
  deviceName: defaultName(),
  loaded: false,

  status: "unpaired",
  error: null,
  retryIn: 0,
  desktopHost: "",
  desktopVersion: "",
  snap: null,

  load: async () => {
    try {
      const raw = await AsyncStorage.getItem(KEY);
      const v = raw ? (JSON.parse(raw) as Partial<ConnState>) : {};
      set({
        host: v.host ?? "",
        port: v.port ?? 6768,
        token: v.token ?? "",
        deviceName: v.deviceName || defaultName(),
        loaded: true,
        status: v.host && v.token ? "offline" : "unpaired",
      });
    } catch {
      set({ loaded: true, status: "unpaired" });
    }
  },

  pair: async ({ host, port, token }) => {
    set({ host, port, token, status: "offline", error: null, snap: null });
    await persist(get());
  },

  forget: async () => {
    set({ host: "", port: 6768, token: "", status: "unpaired", error: null, snap: null });
    await AsyncStorage.removeItem(KEY);
  },

  setDeviceName: async (deviceName) => {
    set({ deviceName: deviceName.trim() || defaultName() });
    await persist(get());
  },

  connecting: () => set({ status: "connecting", error: null, retryIn: 0 }),
  online: ({ host, version }) =>
    set({ status: "online", error: null, retryIn: 0, desktopHost: host, desktopVersion: version }),
  offline: () => set((s) => (s.status === "failed" ? {} : { status: "offline" })),
  retrying: (secs) => set((s) => (s.status === "failed" ? {} : { status: "retrying", retryIn: secs })),
  failed: (error) => set({ status: "failed", error }),
  setSnap: (snap) => set({ snap }),
}));

async function persist(s: ConnState) {
  const { host, port, token, deviceName } = s;
  await AsyncStorage.setItem(KEY, JSON.stringify({ host, port, token, deviceName }));
}

// ---- selectors ----

export function useSpace(id: string | undefined): Space | null {
  return useConn((s) => s.snap?.spaces.find((w) => w.id === id) ?? null);
}

export function usePane(id: string | undefined): { pane: Pane; space: Space } | null {
  return useConn((s) => {
    for (const w of s.snap?.spaces ?? []) {
      const p = w.panes.find((x) => x.id === id);
      if (p) return { pane: p, space: w };
    }
    return null;
  });
}

/** how many panes across everything are waiting on you — the number worth putting on a badge */
export function useWaitingCount(): number {
  return useConn(
    (s) => s.snap?.spaces.reduce((n, w) => n + w.panes.filter((p) => p.state === "waiting").length, 0) ?? 0,
  );
}

/** parse a `hyprspace://pair?host=…&port=…&token=…` payload (the desktop's QR / copied link) */
export function parsePairing(text: string): { host: string; port: number; token: string } | null {
  const s = text.trim();
  const qs = s.includes("?") ? s.slice(s.indexOf("?") + 1) : s;
  const out: Record<string, string> = {};
  for (const part of qs.split("&")) {
    const [k, v = ""] = part.split("=");
    if (k) out[decodeURIComponent(k)] = decodeURIComponent(v);
  }
  const host = out.host?.trim();
  const token = out.token?.trim();
  const port = Number(out.port);
  if (!host || !token || token.length < 16) return null;
  return { host, port: Number.isFinite(port) && port > 0 ? port : 6768, token };
}
