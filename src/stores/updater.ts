import { create } from "zustand";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { killAllPtys } from "../api";

export type UpdatePhase =
  | "idle"
  | "checking"
  | "available"
  | "uptodate"
  | "downloading"
  | "error";

interface UpdaterState {
  phase: UpdatePhase;
  update: Update | null;
  detail: string; // version, progress text, or error message
  pct: number; // download progress 0–100, or -1 for indeterminate
  checkNow: (silent?: boolean) => Promise<void>;
  install: () => Promise<void>;
  dismiss: () => void;
}

/**
 * Say what actually went wrong. "Update failed — try again" was useless: retrying doesn't help when
 * the installer couldn't replace a running binary, which is the common case here — panes keep child
 * processes (the agent CLIs, PowerShell, OpenConsole.exe) holding the install folder open.
 */
function installError(e: unknown): string {
  const msg = String((e as { message?: string })?.message ?? e);
  if (/signature|verif|corrupt|malformed/i.test(msg)) {
    return "That download didn't verify — it may be incomplete. Try again.";
  }
  if (/permission|denied|access|busy|in use|os error 5|os error 32/i.test(msg)) {
    return "Couldn't replace the app while it's running. Close every pane and try again.";
  }
  if (/network|connect|timed? ?out|dns|tls|certificate/i.test(msg)) {
    return "Couldn't download the update — check your connection.";
  }
  // anything unrecognised: show it rather than hide it. A cryptic message still beats a useless one.
  return msg.length > 3 && msg !== "undefined" ? `Update failed: ${trimMsg(msg)}` : "Update failed — try again";
}

const trimMsg = (s: string) => (s.length > 120 ? s.slice(0, 119) + "…" : s);

export const useUpdater = create<UpdaterState>()((set, get) => ({
  phase: "idle",
  update: null,
  detail: "",
  pct: -1,

  // silent = the automatic check (launch/interval/focus): a failure just goes quiet. only an explicit
  // "Check now" surfaces an error toast, so a transient blip on startup never nags.
  checkNow: async (silent = false) => {
    const p = get().phase;
    if (p === "checking" || p === "downloading") return;
    set({ phase: "checking", detail: "" });
    try {
      const u = await check();
      if (u?.available) set({ phase: "available", update: u, detail: u.version });
      else set({ phase: "uptodate", update: null, detail: "" });
    } catch (e) {
      // no manifest entry for this platform = there's simply no update channel for us (macOS ships as
      // a manual .dmg with no updater artifacts) — that's "up to date", never a server error
      const msg = String((e as { message?: string })?.message ?? e);
      if (/platform|target/i.test(msg)) {
        set({ phase: "uptodate", update: null, detail: "" });
        return;
      }
      console.error("update check failed:", e);
      set(silent ? { phase: "idle", detail: "" } : { phase: "error", detail: "Couldn't reach the update server" });
    }
  },

  install: async () => {
    if (get().phase === "downloading") return;
    set({ phase: "downloading", detail: "Checking for the latest version…", pct: -1 });
    // Re-check at the moment of install so an app behind by several releases jumps straight to the
    // newest version in ONE hop. The endpoint always serves the latest release's manifest, so the
    // cached toast (which may name an older version if more shipped since we last checked) never
    // forces a two-step update. Fall back to the cached update only if the re-check can't reach the net.
    let u = get().update;
    try {
      u = await check(); // fresh truth: the newest Update, or null if we're already current
    } catch {
      /* offline re-check — keep the cached update and try it */
    }
    if (!u) {
      set({ phase: "uptodate", update: null, detail: "", pct: -1 });
      return;
    }
    set({ detail: `Preparing v${u.version}…`, pct: -1 });
    try {
      let total = 0;
      let got = 0;
      // Download and install as SEPARATE steps, so the PTYs can be torn down in between.
      // The installer replaces hyprspace-tauri.exe in place and can't while the panes' ConPTY hosts
      // (OpenConsole.exe) hold the install folder open — that's what made this fail with a busy
      // workspace. Killing them only after the bytes have landed means a failed download never
      // costs you your panes for nothing.
      await u.download((e) => {
        if (e.event === "Started") {
          total = e.data.contentLength ?? 0;
          set({ detail: "Downloading…", pct: total ? 0 : -1 });
        } else if (e.event === "Progress") {
          got += e.data.chunkLength;
          const p = total ? Math.round((got / total) * 100) : -1;
          set({ detail: total ? `Downloading ${p}%` : "Downloading…", pct: p });
        }
      });
      set({ detail: "Closing panes…", pct: -1 });
      // the sessions themselves are persisted and come back on relaunch (claude panes --resume)
      await killAllPtys().catch(() => {}); // best-effort: a failure here shouldn't block the install
      set({ detail: "Installing…", pct: -1 });
      await u.install();
      set({ detail: "Restarting…", pct: -1 });
      await relaunch();
    } catch (e) {
      console.error("update failed:", e);
      set({ phase: "error", detail: installError(e), pct: -1 });
    }
  },

  dismiss: () =>
    set((s) => (s.phase === "available" || s.phase === "error" ? { ...s, phase: "idle" } : s)),
}));
