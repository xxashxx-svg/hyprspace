import { create } from "zustand";

interface UiState {
  view: "home" | "space"; // home dashboard vs a workspace
  railCollapsed: boolean;
  maximizedId: string | null; // session id of the zoomed-to-fullscreen pane
  fileDropId: string | null; // session id of the pane a file drag is currently over
  skillDropId: string | null; // pane a skill chip is being dragged over
  settingsOpen: boolean; // in-app settings screen
  settingsTab: string;
  newProjectOpen: boolean; // the New Project wizard
  paletteOpen: boolean;
  dockOpen: boolean;
  dockTab: "changes" | "skills";
  paneDragging: boolean; // a terminal pane is mid-drag (rail shows spaces as drop targets)
  paneDragOverWs: string | null; // the space the dragged pane is hovering over in the rail
  goHome: () => void;
  goSpace: () => void;
  toggleRail: () => void;
  toggleMaximized: (id: string) => void;
  clearMaximized: () => void;
  setFileDrop: (id: string | null) => void;
  setSkillDrop: (id: string | null) => void;
  openSettings: (tab?: string) => void;
  closeSettings: () => void;
  setSettingsTab: (t: string) => void;
  openNewProject: () => void;
  closeNewProject: () => void;
  togglePalette: () => void;
  setPalette: (b: boolean) => void;
  toggleDock: () => void;
  setDock: (b: boolean) => void;
  setDockTab: (t: "changes" | "skills") => void;
  setPaneDrag: (on: boolean) => void;
  setPaneDragOverWs: (id: string | null) => void;
}

export const useUi = create<UiState>()((set) => ({
  view: "home", // land on the dashboard
  railCollapsed: false,
  maximizedId: null,
  fileDropId: null,
  skillDropId: null,
  settingsOpen: false,
  settingsTab: "appearance",
  newProjectOpen: false,
  paletteOpen: false,
  dockOpen: false,
  dockTab: "skills",
  paneDragging: false,
  paneDragOverWs: null,
  goHome: () => set({ view: "home" }),
  goSpace: () => set({ view: "space" }),
  toggleRail: () => set((s) => ({ railCollapsed: !s.railCollapsed })),
  toggleMaximized: (id) => set((s) => ({ maximizedId: s.maximizedId === id ? null : id })),
  clearMaximized: () => set({ maximizedId: null }),
  setFileDrop: (id) => set({ fileDropId: id }),
  setSkillDrop: (id) => set({ skillDropId: id }),
  openSettings: (tab) => set((s) => ({ settingsOpen: true, settingsTab: tab ?? s.settingsTab })),
  closeSettings: () => set({ settingsOpen: false }),
  setSettingsTab: (t) => set({ settingsTab: t }),
  openNewProject: () => set({ newProjectOpen: true }),
  closeNewProject: () => set({ newProjectOpen: false }),
  togglePalette: () => set((s) => ({ paletteOpen: !s.paletteOpen })),
  setPalette: (b) => set({ paletteOpen: b }),
  toggleDock: () => set((s) => ({ dockOpen: !s.dockOpen })),
  setDock: (b) => set({ dockOpen: b }),
  setDockTab: (t) => set({ dockTab: t, dockOpen: true }),
  setPaneDrag: (on) =>
    set(on ? { paneDragging: true } : { paneDragging: false, paneDragOverWs: null }),
  setPaneDragOverWs: (id) => set({ paneDragOverWs: id }),
}));
