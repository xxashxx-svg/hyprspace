import { create } from "zustand";

interface UiState {
  railCollapsed: boolean;
  maximizedId: string | null; // session id of the zoomed-to-fullscreen pane
  fileDropId: string | null; // session id of the pane a file drag is currently over
  settingsOpen: boolean;
  settingsTab: string; // active tab in the settings window
  paletteOpen: boolean;
  dockOpen: boolean;
  dockTab: "changes" | "run";
  toggleRail: () => void;
  toggleMaximized: (id: string) => void;
  clearMaximized: () => void;
  setFileDrop: (id: string | null) => void;
  toggleSettings: () => void;
  openSettings: (tab?: string) => void;
  setSettingsTab: (t: string) => void;
  togglePalette: () => void;
  setPalette: (b: boolean) => void;
  toggleDock: () => void;
  setDock: (b: boolean) => void;
  setDockTab: (t: "changes" | "run") => void;
}

export const useUi = create<UiState>()((set) => ({
  railCollapsed: false,
  maximizedId: null,
  fileDropId: null,
  settingsOpen: false,
  settingsTab: "appearance",
  paletteOpen: false,
  dockOpen: false,
  dockTab: "changes",
  toggleRail: () => set((s) => ({ railCollapsed: !s.railCollapsed })),
  toggleMaximized: (id) => set((s) => ({ maximizedId: s.maximizedId === id ? null : id })),
  clearMaximized: () => set({ maximizedId: null }),
  setFileDrop: (id) => set({ fileDropId: id }),
  toggleSettings: () => set((s) => ({ settingsOpen: !s.settingsOpen })),
  openSettings: (tab) => set((s) => ({ settingsOpen: true, settingsTab: tab ?? s.settingsTab })),
  setSettingsTab: (t) => set({ settingsTab: t }),
  togglePalette: () => set((s) => ({ paletteOpen: !s.paletteOpen })),
  setPalette: (b) => set({ paletteOpen: b }),
  toggleDock: () => set((s) => ({ dockOpen: !s.dockOpen })),
  setDock: (b) => set({ dockOpen: b }),
  setDockTab: (t) => set({ dockTab: t, dockOpen: true }),
}));
