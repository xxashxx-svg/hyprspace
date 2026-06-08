import { create } from "zustand";

interface UiState {
  railCollapsed: boolean;
  maximizedId: string | null; // session id of the zoomed-to-fullscreen pane
  fileDropId: string | null; // session id of the pane a file drag is currently over
  settingsOpen: boolean;
  toggleRail: () => void;
  toggleMaximized: (id: string) => void;
  clearMaximized: () => void;
  setFileDrop: (id: string | null) => void;
  toggleSettings: () => void;
}

export const useUi = create<UiState>()((set) => ({
  railCollapsed: false,
  maximizedId: null,
  fileDropId: null,
  settingsOpen: false,
  toggleRail: () => set((s) => ({ railCollapsed: !s.railCollapsed })),
  toggleMaximized: (id) => set((s) => ({ maximizedId: s.maximizedId === id ? null : id })),
  clearMaximized: () => set({ maximizedId: null }),
  setFileDrop: (id) => set({ fileDropId: id }),
  toggleSettings: () => set((s) => ({ settingsOpen: !s.settingsOpen })),
}));
