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

/**
 * One way to reach the desktop. There are usually two: the LAN address (fast, and what you want at
 * home) and a way in from outside — a VPN address, or a tunnel's public `wss://` hostname. The app
 * tries them in order and remembers which one worked, so walking out of the house just costs one
 * failed attempt rather than a settings trip.
 */
export interface Endpoint {
  /** "Home wifi", "Tailscale", "Tunnel" — shown in Settings so you know where you're connected */
  label: string;
  host: string;
  port: number;
  /** wss:// instead of ws:// — required for a public tunnel, pointless on a LAN */
  tls?: boolean;
}

export function endpointUrl(e: Endpoint): string {
  const scheme = e.tls ? "wss" : "ws";
  // a tunnel on the default TLS port has no port in its URL; anything else is explicit
  const port = e.tls && e.port === 443 ? "" : `:${e.port}`;
  return `${scheme}://${e.host}${port}/`;
}

interface ConnState {
  // paired desktop
  host: string;
  port: number;
  token: string;
  deviceName: string;
  /** every known way in, best-first. Index 0 is whatever last worked. */
  endpoints: Endpoint[];
  /** which endpoint the live connection is using, for Settings to show */
  activeEndpoint: string;
  loaded: boolean;

  status: Status;
  error: string | null;
  retryIn: number;
  desktopHost: string;
  desktopVersion: string;
  /** the protocol the desktop speaks; higher than ours = a newer app exists */
  desktopProtocol: number;
  snap: Snap | null;

  load: () => Promise<void>;
  pair: (p: { host: string; port: number; token: string; endpoints?: Endpoint[] }) => Promise<void>;
  forget: () => Promise<void>;
  setDeviceName: (n: string) => Promise<void>;
  /** add or replace a way in (the "reach it from outside" field in Settings) */
  setEndpoint: (e: Endpoint) => Promise<void>;
  removeEndpoint: (label: string) => Promise<void>;
  /** promote whatever just connected, so the next launch tries it first */
  preferEndpoint: (label: string) => Promise<void>;

  connecting: () => void;
  online: (d: { host: string; version: string; desktopProtocol: number }) => void;
  setActiveEndpoint: (label: string) => void;
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
  endpoints: [],
  activeEndpoint: "",
  loaded: false,

  status: "unpaired",
  error: null,
  retryIn: 0,
  desktopHost: "",
  desktopVersion: "",
  desktopProtocol: 0,
  snap: null,

  load: async () => {
    try {
      const raw = await AsyncStorage.getItem(KEY);
      const v = raw ? (JSON.parse(raw) as Partial<ConnState>) : {};
      // a build before endpoints existed saved a bare host/port — carry it over as the LAN entry
      const endpoints =
        v.endpoints?.length
          ? v.endpoints
          : v.host
            ? [{ label: "Local network", host: v.host, port: v.port ?? 6768 }]
            : [];
      set({
        host: v.host ?? endpoints[0]?.host ?? "",
        port: v.port ?? endpoints[0]?.port ?? 6768,
        token: v.token ?? "",
        deviceName: v.deviceName || defaultName(),
        endpoints,
        loaded: true,
        status: endpoints.length && v.token ? "offline" : "unpaired",
      });
    } catch {
      set({ loaded: true, status: "unpaired" });
    }
  },

  pair: async ({ host, port, token, endpoints }) => {
    const list = endpoints?.length ? endpoints : [{ label: "Local network", host, port }];
    set({ host, port, token, endpoints: list, status: "offline", error: null, snap: null });
    await persist(get());
  },

  forget: async () => {
    set({
      host: "", port: 6768, token: "", endpoints: [], activeEndpoint: "",
      status: "unpaired", error: null, snap: null,
    });
    await AsyncStorage.removeItem(KEY);
  },

  setDeviceName: async (deviceName) => {
    set({ deviceName: deviceName.trim() || defaultName() });
    await persist(get());
  },

  setEndpoint: async (e) => {
    set((s) => ({ endpoints: [...s.endpoints.filter((x) => x.label !== e.label), e] }));
    await persist(get());
  },

  removeEndpoint: async (label) => {
    set((s) => ({ endpoints: s.endpoints.filter((x) => x.label !== label) }));
    await persist(get());
  },

  preferEndpoint: async (label) => {
    const { endpoints } = get();
    if (endpoints[0]?.label === label) return; // already first, nothing to save
    const hit = endpoints.find((e) => e.label === label);
    if (!hit) return;
    set({ endpoints: [hit, ...endpoints.filter((e) => e.label !== label)] });
    await persist(get());
  },

  connecting: () => set({ status: "connecting", error: null, retryIn: 0 }),
  online: ({ host, version, desktopProtocol }) =>
    set({
      status: "online", error: null, retryIn: 0,
      desktopHost: host, desktopVersion: version, desktopProtocol,
    }),
  setActiveEndpoint: (activeEndpoint) => set({ activeEndpoint }),
  offline: () => set((s) => (s.status === "failed" ? {} : { status: "offline" })),
  retrying: (secs) => set((s) => (s.status === "failed" ? {} : { status: "retrying", retryIn: secs })),
  failed: (error) => set({ status: "failed", error }),
  setSnap: (snap) => set({ snap }),
}));

async function persist(s: ConnState) {
  const { host, port, token, deviceName, endpoints } = s;
  await AsyncStorage.setItem(KEY, JSON.stringify({ host, port, token, deviceName, endpoints }));
}

// ---- selectors ----

export function useSpace(id: string | undefined): Space | null {
  return useConn((s) => s.snap?.spaces.find((w) => w.id === id) ?? null);
}

/**
 * Two selectors, not one returning `{ pane, space }`.
 *
 * Zustand compares what a selector returns with Object.is to decide whether to re-render. A selector
 * that builds a new object every call never matches its own previous result, so React re-renders
 * forever and throws — which crashed the terminal screen. Each selector here returns an object that
 * already exists in the snapshot, so the reference is stable between updates.
 */
export function usePane(id: string | undefined): { pane: Pane; space: Space } | null {
  const pane = useConn(
    (s) => s.snap?.spaces.flatMap((w) => w.panes).find((p) => p.id === id) ?? null,
  );
  const space = useConn(
    (s) => s.snap?.spaces.find((w) => w.panes.some((p) => p.id === id)) ?? null,
  );
  // building this object per render is fine — it's a hook's return value, not a store snapshot
  return pane && space ? { pane, space } : null;
}

/** how many panes across everything are waiting on you — the number worth putting on a badge */
export function useWaitingCount(): number {
  return useConn(
    (s) => s.snap?.spaces.reduce((n, w) => n + w.panes.filter((p) => p.state === "waiting").length, 0) ?? 0,
  );
}

/**
 * Is this something we can safely put in a `ws://` URL?
 *
 * This is a guard, not a nicety. React Native's WebSocketModule builds the Origin header by handing
 * the URL to java.net.URI, and a URL it can't parse throws IllegalArgumentException on a native
 * background thread — which takes the whole app down. That throw happens after `new WebSocket()`
 * returns, so no JS try/catch can catch it. The only defence is to never build a bad URL.
 */
export function validHost(h: string): boolean {
  if (!h || h.length > 255) return false;
  // IPv6 has to be bracketed in a URL; anything else is a hostname or IPv4
  if (h.startsWith("[")) return /^\[[0-9a-fA-F:.]+\]$/.test(h);
  return /^[a-zA-Z0-9.-]+$/.test(h) && !h.startsWith(".") && !h.endsWith(".");
}

/**
 * Parse a `hyprspace://pair?host=…&port=…&token=…` payload (the desktop's QR / copied link).
 *
 * An optional `remote=` carries a second way in for when you're not on the same network — either a
 * VPN address (`100.x.x.x:6768`) or a tunnel's public hostname (`wss://x.trycloudflare.com`). Older
 * desktops don't send it, and the LAN entry alone still works.
 */
export function parsePairing(
  text: string,
): { host: string; port: number; token: string; endpoints: Endpoint[] } | null {
  const s = text.trim();
  const qs = s.includes("?") ? s.slice(s.indexOf("?") + 1) : s;
  const out: Record<string, string> = {};
  for (const part of qs.split("&")) {
    const [k, v = ""] = part.split("=");
    if (k) out[decodeURIComponent(k)] = decodeURIComponent(v);
  }
  // tolerate a pasted "http://1.2.3.4:6768/" or a stray backslash from a shell/QR round-trip, then
  // insist on what's left being a real host
  let host = (out.host ?? "")
    .trim()
    .replace(/\\/g, "")
    .replace(/^[a-zA-Z]+:\/\//, "")
    .replace(/\/.*$/, "");
  let port = Number(out.port);
  const colon = host.lastIndexOf(":");
  if (!host.startsWith("[") && colon > 0) {
    // an address pasted as "1.2.3.4:6768" — take the port from it when none was given separately
    const inline = Number(host.slice(colon + 1));
    if (Number.isFinite(inline) && inline > 0) port = port || inline;
    host = host.slice(0, colon);
  }
  const token = out.token?.trim();
  if (!validHost(host) || !token || token.length < 16) return null;
  const lanPort = Number.isFinite(port) && port > 0 && port < 65536 ? port : 6768;

  const endpoints: Endpoint[] = [{ label: "Local network", host, port: lanPort }];
  const remote = parseEndpoint(out.remote ?? "", "Away");
  if (remote) endpoints.push(remote);
  return { host, port: lanPort, token, endpoints };
}

/**
 * Turn something a person typed into an endpoint: `100.90.1.2`, `1.2.3.4:6768`,
 * `wss://box.example.com`, `https://x.trycloudflare.com` — all mean something sensible.
 */
export function parseEndpoint(text: string, label: string): Endpoint | null {
  let s = text.trim().replace(/\\/g, "");
  if (!s) return null;
  const scheme = /^([a-zA-Z]+):\/\//.exec(s)?.[1]?.toLowerCase();
  const tls = scheme === "wss" || scheme === "https";
  s = s.replace(/^[a-zA-Z]+:\/\//, "").replace(/\/.*$/, "");

  let port = tls ? 443 : 6768;
  const colon = s.lastIndexOf(":");
  if (!s.startsWith("[") && colon > 0) {
    const n = Number(s.slice(colon + 1));
    if (Number.isFinite(n) && n > 0 && n < 65536) port = n;
    s = s.slice(0, colon);
  }
  if (!validHost(s)) return null;
  return { label, host: s, port, ...(tls ? { tls: true } : {}) };
}
