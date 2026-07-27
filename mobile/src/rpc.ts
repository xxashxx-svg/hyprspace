// The WebSocket client that talks to HyprSpace on the desktop (src-tauri/src/bridge.rs).
//
// One socket for everything: a pushed state mirror, live PTY bytes for whatever panes you're
// watching, and request/response calls for anything that needs the desktop app itself. Reconnects on
// its own with backoff, because a phone loses wifi constantly.

import { endpointUrl, useConn, validHost, type Snap } from "./store";

/** Must match PROTOCOL in bridge.rs — a mismatch is reported instead of half-working. */
export const PROTOCOL = 1;

const REQ_TIMEOUT = 20_000;
const PING_EVERY = 25_000;
// how long one address gets before we try the next. A wrong-network address doesn't refuse — it
// hangs until the OS gives up, which is far too long to sit through when you've just left the house.
const DIAL_TIMEOUT = 6_000;

export interface PaneHandlers {
  onData: (bytes: Uint8Array) => void;
  onSize?: (cols: number, rows: number) => void;
  onExit?: (code: number) => void;
  /** the desktop has no live PTY for this pane (space not opened, or the pane is gone) */
  onGone?: () => void;
}

let ws: WebSocket | null = null;
let reqSeq = 1;
let retries = 0;
let retryTimer: ReturnType<typeof setTimeout> | null = null;
let pingTimer: ReturnType<typeof setInterval> | null = null;
let wanted = false; // false = the user disconnected; don't reconnect behind their back
let attempt = 0; // which endpoint to dial next — advanced on every failed connection

const pending = new Map<number, { ok: (v: unknown) => void; fail: (e: Error) => void; timer: ReturnType<typeof setTimeout> }>();
const panes = new Map<string, Set<PaneHandlers>>();

const dec = new TextDecoder();

// --- base64 for keystrokes out / pane bytes in. RN has atob/btoa but not on every engine version,
// and we need bytes either way, so do it by hand. ---
const B64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

export function toBase64(bytes: Uint8Array): string {
  let out = "";
  for (let i = 0; i < bytes.length; i += 3) {
    const a = bytes[i];
    const b = bytes[i + 1];
    const cc = bytes[i + 2];
    out += B64[a >> 2];
    out += B64[((a & 3) << 4) | ((b ?? 0) >> 4)];
    out += b === undefined ? "=" : B64[((b & 15) << 2) | ((cc ?? 0) >> 6)];
    out += cc === undefined ? "=" : B64[cc & 63];
  }
  return out;
}

function send(obj: unknown) {
  if (ws?.readyState === 1) ws.send(JSON.stringify(obj));
}

function failAllPending(reason: string) {
  for (const [, p] of pending) {
    clearTimeout(p.timer);
    p.fail(new Error(reason));
  }
  pending.clear();
}

/** Ask the desktop for something. Rejects on error, on timeout, or if the socket goes away. */
export function req<T = unknown>(m: string, p: Record<string, unknown> = {}): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    if (ws?.readyState !== 1) return reject(new Error("not connected"));
    const id = reqSeq++;
    const timer = setTimeout(() => {
      pending.delete(id);
      reject(new Error("the desktop didn't answer"));
    }, REQ_TIMEOUT);
    pending.set(id, { ok: (v) => resolve(v as T), fail: reject, timer });
    send({ t: "req", id, m, p });
  });
}

/** Watch a pane's terminal output. Returns an unsubscriber. */
export function watchPane(pane: string, h: PaneHandlers): () => void {
  let set = panes.get(pane);
  if (!set) {
    set = new Set();
    panes.set(pane, set);
    send({ t: "sub", pane });
  }
  set.add(h);
  return () => {
    const s = panes.get(pane);
    if (!s) return;
    s.delete(h);
    if (s.size === 0) {
      panes.delete(pane);
      send({ t: "unsub", pane });
    }
  };
}

/**
 * Ask for a pane's replay again. `sub` is idempotent on the desktop, so this just re-sends the recent
 * tail — what you need when the terminal you were drawing into got thrown away (Android reclaiming a
 * WebView) and would otherwise sit blank until the next byte of output.
 */
export function resyncPane(pane: string) {
  send({ t: "sub", pane });
}

/** Type into a pane. `text` goes through as-is — control bytes included. */
export function writePane(pane: string, text: string) {
  send({ t: "in", pane, d: toBase64(new TextEncoder().encode(text)) });
}

function handleBinary(buf: ArrayBuffer) {
  const all = new Uint8Array(buf);
  if (all.length < 1) return;
  const idLen = all[0];
  if (all.length < 1 + idLen) return;
  const pane = dec.decode(all.subarray(1, 1 + idLen));
  const data = all.subarray(1 + idLen);
  const set = panes.get(pane);
  if (!set) return;
  for (const h of set) h.onData(data);
}

function handleText(raw: string) {
  let m: Record<string, unknown>;
  try {
    m = JSON.parse(raw);
  } catch {
    return;
  }
  const pane = typeof m.pane === "string" ? m.pane : "";
  switch (m.t) {
    case "welcome": {
      retries = 0;
      const conn = useConn.getState();
      conn.online({
        host: String(m.host ?? "desktop"),
        version: String(m.version ?? ""),
        // the desktop speaks a newer protocol but still accepts us — everything works, there's just
        // a newer app to install. Worth surfacing, not worth blocking on.
        desktopProtocol: Number(m.protocol ?? PROTOCOL),
      });
      // this one works — put it first so the next launch doesn't retry a dead address
      void conn.preferEndpoint(conn.activeEndpoint);
      attempt = 0;
      // re-arm any pane we were watching before the socket dropped
      for (const p of panes.keys()) send({ t: "sub", pane: p });
      break;
    }

    case "state":
      useConn.getState().setSnap((m.d as Snap | null) ?? null);
      break;

    case "res": {
      const id = Number(m.id ?? 0);
      const p = pending.get(id);
      if (!p) return;
      pending.delete(id);
      clearTimeout(p.timer);
      if (m.ok) p.ok(m.d);
      else {
        const d = (m.d ?? {}) as { error?: string };
        p.fail(new Error(d.error || "the desktop refused that"));
      }
      break;
    }

    case "size":
      panes.get(pane)?.forEach((h) => h.onSize?.(Number(m.cols ?? 80), Number(m.rows ?? 24)));
      break;

    case "exit":
      panes.get(pane)?.forEach((h) => h.onExit?.(Number(m.code ?? 0)));
      break;

    case "gone":
      panes.get(pane)?.forEach((h) => h.onGone?.());
      break;

    case "error": {
      const code = String(m.code ?? "");
      wanted = false; // both of these are permanent — retrying just spins
      useConn.getState().failed(
        code === "auth"
          ? "That pairing code was rejected. Re-scan the QR in Settings → Mobile on the desktop."
          : code === "protocol"
            ? m.stale === "desktop"
              ? "Your desktop HyprSpace is older than this app — update it and try again."
              : "This app is too old for that desktop. Install the newer APK from its releases page."
            : "The desktop refused the connection.",
      );
      ws?.close();
      break;
    }
  }
}

function scheduleRetry() {
  if (!wanted || retryTimer) return;
  // 1s, 2s, 4s… capped at 15s — fast enough to feel instant when you walk back into wifi
  const wait = Math.min(15_000, 1000 * 2 ** Math.min(retries, 4));
  retries++;
  useConn.getState().retrying(Math.round(wait / 1000));
  retryTimer = setTimeout(() => {
    retryTimer = null;
    open();
  }, wait);
}

function open() {
  const { endpoints, token, deviceName } = useConn.getState();
  if (!endpoints.length || !token) return;

  // Never hand a malformed URL to the native WebSocket — it throws on a background thread and kills
  // the app (see validHost). A value saved by an older build could still be bad, so check here too.
  const usable = endpoints.filter((e) => validHost(e.host) && e.port > 0 && e.port < 65536);
  if (!usable.length) {
    wanted = false;
    useConn.getState().failed("None of the saved addresses are valid. Re-pair with the QR on your desktop.");
    return;
  }

  // Walk the list rather than hammering one address: on the LAN the first entry connects instantly,
  // and away from home it fails fast and we fall through to the VPN/tunnel entry.
  const target = usable[attempt % usable.length];
  cleanupSocket();
  useConn.getState().connecting();
  useConn.getState().setActiveEndpoint(target.label);

  const sock = new WebSocket(endpointUrl(target));
  sock.binaryType = "arraybuffer";
  ws = sock;

  // A wrong-network address doesn't refuse, it hangs until the OS gives up (~30s+ on Android), which
  // would make walking out of the house feel broken. Give each one a short go, then try the next.
  const dial = setTimeout(() => {
    if (ws === sock && sock.readyState === 0) {
      sock.close(); // onclose advances `attempt`
    }
  }, DIAL_TIMEOUT);

  sock.onopen = () => {
    clearTimeout(dial);
    sock.send(JSON.stringify({ t: "hello", token, protocol: PROTOCOL, name: deviceName }));
    pingTimer = setInterval(() => send({ t: "ping" }), PING_EVERY);
  };
  sock.onmessage = (e) => {
    if (typeof e.data === "string") handleText(e.data);
    else handleBinary(e.data as ArrayBuffer);
  };
  sock.onerror = () => {
    // RN gives no useful detail here; onclose follows and drives the retry
  };
  sock.onclose = () => {
    clearTimeout(dial);
    if (ws === sock) ws = null;
    if (pingTimer) {
      clearInterval(pingTimer);
      pingTimer = null;
    }
    failAllPending("lost the connection");
    if (wanted) {
      attempt++; // next dial tries the other way in
      useConn.getState().offline();
      scheduleRetry();
    }
  };
}

function cleanupSocket() {
  if (!ws) return;
  const old = ws;
  ws = null;
  old.onopen = old.onmessage = old.onerror = old.onclose = null;
  try {
    old.close();
  } catch {
    // already gone
  }
  if (pingTimer) {
    clearInterval(pingTimer);
    pingTimer = null;
  }
}

/** Connect (or reconnect now) using whatever's in the store. */
export function connect() {
  wanted = true;
  retries = 0;
  if (retryTimer) {
    clearTimeout(retryTimer);
    retryTimer = null;
  }
  open();
}

export function disconnect() {
  wanted = false;
  if (retryTimer) {
    clearTimeout(retryTimer);
    retryTimer = null;
  }
  cleanupSocket();
  failAllPending("disconnected");
  panes.clear();
  useConn.getState().offline();
}

export function isOpen() {
  return ws?.readyState === 1;
}
