import { create } from "zustand";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

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
  checkNow: () => Promise<void>;
  install: () => Promise<void>;
  dismiss: () => void;
}

export const useUpdater = create<UpdaterState>()((set, get) => ({
  phase: "idle",
  update: null,
  detail: "",

  checkNow: async () => {
    const p = get().phase;
    if (p === "checking" || p === "downloading") return;
    set({ phase: "checking", detail: "" });
    try {
      const u = await check();
      if (u?.available) set({ phase: "available", update: u, detail: u.version });
      else set({ phase: "uptodate", update: null, detail: "" });
    } catch (e) {
      console.error("update check failed:", e);
      set({ phase: "error", detail: "couldn't reach the update server" });
    }
  },

  install: async () => {
    if (get().phase === "downloading") return;
    set({ phase: "downloading", detail: "checking…" });
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
      set({ phase: "uptodate", update: null, detail: "" });
      return;
    }
    set({ update: u, detail: u.version });
    try {
      let total = 0;
      let got = 0;
      await u.downloadAndInstall((e) => {
        if (e.event === "Started") total = e.data.contentLength ?? 0;
        else if (e.event === "Progress") {
          got += e.data.chunkLength;
          set({ detail: total ? `downloading ${Math.round((got / total) * 100)}%` : "downloading…" });
        } else if (e.event === "Finished") set({ detail: "installing…" });
      });
      await relaunch();
    } catch (e) {
      console.error("update failed:", e);
      set({ phase: "error", detail: "update failed — try again" });
    }
  },

  dismiss: () => set((s) => (s.phase === "available" ? { ...s, phase: "idle" } : s)),
}));
