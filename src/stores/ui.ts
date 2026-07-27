import { create } from "zustand";

interface UiState {
  view: "home" | "space" | "loops" | "launch"; // home dashboard, a workspace, the loops page, or the multi-agent launcher
  launchReturn: "home" | "space" | "loops"; // where the launcher's Cancel/Esc sends you back to
  openLoopId: string | null; // the loop selected in the Loops page master-detail
  railCollapsed: boolean;
  maximizedId: string | null; // session id of the zoomed-to-fullscreen pane
  fileDropId: string | null; // session id of the pane a file drag is currently over
  skillDropId: string | null; // pane a skill chip is being dragged over
  settingsOpen: boolean; // in-app settings screen
  settingsTab: string;
  newProjectOpen: boolean; // the New Project wizard
  paletteOpen: boolean;
  dockOpen: boolean;
  dockTab: "changes" | "skills" | "files";
  paneDragging: boolean; // a terminal pane is mid-drag (rail shows spaces as drop targets)
  paneDragOverWs: string | null; // the space the dragged pane is hovering over in the rail
  goHome: () => void;
  goSpace: () => void;
  goLoops: () => void;
  focusLoop: (id: string) => void; // open a specific loop in the Loops page
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
  openLaunch: () => void;
  closeLaunch: () => void;
  togglePalette: () => void;
  setPalette: (b: boolean) => void;
  toggleDock: () => void;
  setDock: (b: boolean) => void;
  setDockTab: (t: "changes" | "skills" | "files") => void;
  onboardingOpen: boolean;
  openOnboarding: () => void;
  closeOnboarding: () => void;
  setPaneDrag: (on: boolean) => void;
  setPaneDragOverWs: (id: string | null) => void;
}

export const useUi = create<UiState>()((set) => ({
  view: "home", // land on the dashboard
  launchReturn: "home",
  openLoopId: null,
  railCollapsed: false,
  maximizedId: null,
  fileDropId: null,
  skillDropId: null,
  settingsOpen: false,
  settingsTab: "appearance",
  newProjectOpen: false,
  paletteOpen: false,
  dockOpen: false,
  dockTab: "files", // the dock opens on Files — the tab you actually browse from
  paneDragging: false,
  paneDragOverWs: null,
  goHome: () => set({ view: "home" }),
  goSpace: () => set({ view: "space" }),
  goLoops: () => set({ view: "loops" }),
  focusLoop: (id) => set({ view: "loops", openLoopId: id }),
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
  // open the full-page launcher, remembering the page to return to on Cancel/Esc
  openLaunch: () => set((s) => (s.view === "launch" ? {} : { view: "launch", launchReturn: s.view })),
  closeLaunch: () => set((s) => ({ view: s.launchReturn })),
  togglePalette: () => set((s) => ({ paletteOpen: !s.paletteOpen })),
  setPalette: (b) => set({ paletteOpen: b }),
  toggleDock: () => set((s) => ({ dockOpen: !s.dockOpen })),
  setDock: (b) => set({ dockOpen: b }),
  setDockTab: (t) => set({ dockTab: t, dockOpen: true }),
  // closing lands you back on the file tree, since the editor tab disappears with the file
  onboardingOpen: false,
  openOnboarding: () => set({ onboardingOpen: true, settingsOpen: false }),
  closeOnboarding: () => set({ onboardingOpen: false }),
  setPaneDrag: (on) =>
    set(on ? { paneDragging: true } : { paneDragging: false, paneDragOverWs: null }),
  setPaneDragOverWs: (id) => set({ paneDragOverWs: id }),
}));
