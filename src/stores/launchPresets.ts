// Saved launcher configs — a folder + grid size + agent mix you can relaunch in one click
// (BridgeMind's "Presets" row). Persisted as "launchPresets".
import { create } from "zustand";
import { saveState, loadState } from "../api";
import type { ClaudePermission } from "./settings";

export interface LaunchPreset {
  id: string;
  name: string;
  folder: string;
  count: number;
  agents: { claude: number; codex: number; gemini: number; terminal: number };
  claudeMode: ClaudePermission;
}

interface PresetState {
  presets: LaunchPreset[];
  loaded: boolean;
  load: () => Promise<void>;
  save: (p: Omit<LaunchPreset, "id">) => void;
  remove: (id: string) => void;
}

export const useLaunchPresets = create<PresetState>()((set, get) => {
  const persist = () => void saveState("launchPresets", JSON.stringify(get().presets)).catch(() => {});
  return {
    presets: [],
    loaded: false,
    load: async () => {
      if (get().loaded) return;
      const raw = await loadState("launchPresets").catch(() => null);
      if (raw) {
        try {
          const arr = JSON.parse(raw);
          if (Array.isArray(arr)) set({ presets: arr as LaunchPreset[] });
        } catch {
          /* ignore a bad blob */
        }
      }
      set({ loaded: true });
    },
    save: (p) => {
      // replace a same-named preset rather than piling up duplicates
      set((s) => {
        const rest = s.presets.filter((x) => x.name !== p.name);
        return { presets: [...rest, { ...p, id: crypto.randomUUID() }] };
      });
      persist();
    },
    remove: (id) => {
      set((s) => ({ presets: s.presets.filter((x) => x.id !== id) }));
      persist();
    },
  };
});
