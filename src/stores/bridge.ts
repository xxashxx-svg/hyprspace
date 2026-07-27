// Mobile bridge settings + live status. The server itself is Rust (src-tauri/src/bridge.rs); this
// owns the bits that belong to the app: whether it's on, which port, and the pairing token.
//
// The token lives here (not in Rust) because the frontend already owns persistence and the pairing
// UI, and `crypto.getRandomValues` is a proper CSPRNG — Rust only ever compares against it.
import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import {
  bridgeStart,
  bridgeStatus,
  bridgeStop,
  loadState,
  saveState,
  type BridgeInfo,
  type BridgePeer,
} from "../api";

export const DEFAULT_BRIDGE_PORT = 6768;

function mintToken(): string {
  const b = new Uint8Array(16);
  crypto.getRandomValues(b);
  return Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
}

interface BridgeState {
  enabled: boolean;
  port: number;
  token: string;
  info: BridgeInfo | null;
  hydrated: boolean;
  setEnabled: (b: boolean) => void;
  setPort: (p: number) => void;
  /** new token = every paired phone has to scan again */
  regenToken: () => void;
  setInfo: (i: BridgeInfo | null) => void;
  hydrate: (partial: Partial<BridgeState>) => void;
}

export const useBridge = create<BridgeState>()((set) => ({
  enabled: false,
  port: DEFAULT_BRIDGE_PORT,
  token: mintToken(),
  info: null,
  hydrated: false,

  setEnabled: (enabled) => set({ enabled }),
  setPort: (port) => set({ port: Number.isFinite(port) && port > 0 ? Math.floor(port) : DEFAULT_BRIDGE_PORT }),
  regenToken: () => set({ token: mintToken() }),
  setInfo: (info) => set({ info }),
  hydrate: (partial) =>
    set((s) => ({
      enabled: partial.enabled ?? s.enabled,
      port: partial.port ?? s.port,
      // an older save has no token — keep the freshly minted one rather than storing ""
      token: partial.token && partial.token.length >= 16 ? partial.token : s.token,
      hydrated: true,
    })),
}));

/** the pairing payload the phone scans — everything it needs to dial home in one string */
export function pairingUrl(info: BridgeInfo | null, token: string, port: number): string {
  const host = info?.address ?? "";
  const p = info?.running ? info.port : port;
  return `hyprspace://pair?host=${encodeURIComponent(host)}&port=${p}&token=${token}`;
}

export function peerLabel(p: BridgePeer): string {
  return p.name || p.addr || "Phone";
}

// Restore the saved settings, then keep the Rust server matched to them and mirror its status back.
// Call once per window; the returned disposer stops the server too.
export async function initBridge(): Promise<() => void> {
  try {
    const raw = await loadState("bridge");
    useBridge.getState().hydrate(raw ? JSON.parse(raw) : {});
  } catch {
    useBridge.getState().hydrate({});
  }

  let last = "";
  const sync = async () => {
    const s = useBridge.getState();
    const sig = `${s.enabled}:${s.port}:${s.token}`;
    if (sig === last) return;
    last = sig;
    void saveState("bridge", JSON.stringify({ enabled: s.enabled, port: s.port, token: s.token }));
    try {
      const info = s.enabled
        ? await bridgeStart(s.port, s.token)
        : (await bridgeStop(), await bridgeStatus());
      useBridge.getState().setInfo(info);
    } catch (e) {
      console.error("mobile bridge:", e);
      useBridge.getState().setInfo(null);
      last = ""; // let a corrected setting retry
    }
  };

  const unsub = useBridge.subscribe(() => void sync());
  void sync();

  // peers connecting/disconnecting — Rust hands us the whole fresh status
  const unlistenP = listen<BridgeInfo>("bridge://peers", (e) => useBridge.getState().setInfo(e.payload));

  return () => {
    unsub();
    void unlistenP.then((u) => u());
    void bridgeStop();
  };
}
