// Per-FOLDER project config: startup tasks, env vars, default shell — keyed by the folder path so
// opening any project at that folder gets the same setup (and we can manage it in Settings without
// the project being open). Persisted as "projectConfigs".
import { create } from "zustand";
import { saveState, loadState } from "../api";

export interface StartupTask {
  id: string;
  name: string;
  command: string; // typed into the pane's shell; "" = a plain terminal
  folder?: string; // subfolder relative to the project folder; "" = the root
  autostart: boolean; // launch automatically when a project at this folder opens
  background?: boolean; // run hidden as a background service (output captured to logs) vs. a pane
}

export interface ProjectConfig {
  startup: StartupTask[];
  env: Record<string, string>;
  defaultShell?: string;
}

export const EMPTY_CONFIG: ProjectConfig = { startup: [], env: {} };

// stable key for a folder path (trim trailing separators)
export function folderKey(path: string): string {
  return (path || "").replace(/[\\/]+$/, "");
}

interface State {
  configs: Record<string, ProjectConfig>;
  loaded: boolean;
  load: () => Promise<void>;
  getConfig: (folder: string) => ProjectConfig;
  setConfig: (folder: string, patch: Partial<ProjectConfig>) => void;
  removeFolder: (folder: string) => void;
}

export const useProjectConfigs = create<State>((set, get) => {
  const persist = () =>
    void saveState("projectConfigs", JSON.stringify(get().configs)).catch(() => {});
  return {
    configs: {},
    loaded: false,

    load: async () => {
      if (get().loaded) return;
      const raw = await loadState("projectConfigs").catch(() => null);
      if (raw) {
        try {
          const c = JSON.parse(raw);
          if (c && typeof c === "object") set({ configs: c as Record<string, ProjectConfig> });
        } catch {
          /* ignore a bad blob */
        }
      }
      set({ loaded: true });
    },

    getConfig: (folder) => get().configs[folderKey(folder)] ?? EMPTY_CONFIG,

    setConfig: (folder, patch) => {
      const k = folderKey(folder);
      if (!k) return;
      set((s) => ({
        configs: { ...s.configs, [k]: { ...EMPTY_CONFIG, ...s.configs[k], ...patch } },
      }));
      persist();
    },

    removeFolder: (folder) => {
      const k = folderKey(folder);
      set((s) => {
        const configs = { ...s.configs };
        delete configs[k];
        return { configs };
      });
      persist();
    },
  };
});
