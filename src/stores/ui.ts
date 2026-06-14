import { create } from "zustand";

interface UiState {
  railCollapsed: boolean;
  maximizedId: string | null; // session id of the zoomed-to-fullscreen pane
  fileDropId: string | null; // session id of the pane a file drag is currently over
  settingsOpen: boolean;
  paletteOpen: boolean;
  toggleRail: () => void;
  toggleMaximized: (id: string) => void;
  clearMaximized: () => void;
  setFileDrop: (id: string | null) => void;
  toggleSettings: () => void;
  togglePalette: () => void;
  setPalette: (b: boolean) => void;
}

export const useUi = create<UiState>()((set) => ({
  railCollapsed: false,
  maximizedId: null,
  fileDropId: null,
  settingsOpen: false,
  paletteOpen: false,
  toggleRail: () => set((s) => ({ railCollapsed: !s.railCollapsed })),
  toggleMaximized: (id) => set((s) => ({ maximizedId: s.maximizedId === id ? null : id })),
  clearMaximized: () => set({ maximizedId: null }),
  setFileDrop: (id) => set({ fileDropId: id }),
  toggleSettings: () => set((s) => ({ settingsOpen: !s.settingsOpen })),
  togglePalette: () => set((s) => ({ paletteOpen: !s.paletteOpen })),
  setPalette: (b) => set({ paletteOpen: b }),
}));
