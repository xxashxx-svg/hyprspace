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
    const u = get().update;
    if (!u) return;
    set({ phase: "downloading", detail: "starting…" });
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
