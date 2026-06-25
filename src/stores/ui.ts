import { create } from "zustand";

interface UiState {
  view: "home" | "space" | "loops"; // home dashboard, a workspace, or the loops/automations page
  railCollapsed: boolean;
  maximizedId: string | null; // session id of the zoomed-to-fullscreen pane
  fileDropId: string | null; // session id of the pane a file drag is currently over
  skillDropId: string | null; // pane a skill chip is being dragged over
  settingsOpen: boolean; // in-app settings screen
  settingsTab: string;
  newProjectOpen: boolean; // the New Project wizard
  launchOpen: boolean; // the multi-agent "Launch workspace" wizard
  servicesFor: { folder: string; wsId: string; name: string } | null; // the Services config modal
  serviceLogsFor: { id: string; name: string } | null; // the background-service log viewer
  paletteOpen: boolean;
  dockOpen: boolean;
  dockTab: "changes" | "skills" | "services" | "files" | "editor";
  openFile: string | null; // absolute path open in the editor tab
  paneDragging: boolean; // a terminal pane is mid-drag (rail shows spaces as drop targets)
  paneDragOverWs: string | null; // the space the dragged pane is hovering over in the rail
  goHome: () => void;
  goSpace: () => void;
  goLoops: () => void;
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
  openServices: (t: { folder: string; wsId: string; name: string }) => void;
  closeServices: () => void;
  openServiceLogs: (t: { id: string; name: string }) => void;
  closeServiceLogs: () => void;
  togglePalette: () => void;
  setPalette: (b: boolean) => void;
  toggleDock: () => void;
  setDock: (b: boolean) => void;
  setDockTab: (t: "changes" | "skills" | "services" | "files" | "editor") => void;
  openInEditor: (path: string) => void;
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
  launchOpen: false,
  servicesFor: null,
  serviceLogsFor: null,
  paletteOpen: false,
  dockOpen: false,
  dockTab: "skills",
  openFile: null,
  paneDragging: false,
  paneDragOverWs: null,
  goHome: () => set({ view: "home" }),
  goSpace: () => set({ view: "space" }),
  goLoops: () => set({ view: "loops" }),
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
  openLaunch: () => set({ launchOpen: true }),
  closeLaunch: () => set({ launchOpen: false }),
  openServices: (t) => set({ servicesFor: t }),
  closeServices: () => set({ servicesFor: null }),
  openServiceLogs: (t) => set({ serviceLogsFor: t }),
  closeServiceLogs: () => set({ serviceLogsFor: null }),
  togglePalette: () => set((s) => ({ paletteOpen: !s.paletteOpen })),
  setPalette: (b) => set({ paletteOpen: b }),
  toggleDock: () => set((s) => ({ dockOpen: !s.dockOpen })),
  setDock: (b) => set({ dockOpen: b }),
  setDockTab: (t) => set({ dockTab: t, dockOpen: true }),
  openInEditor: (path) => set({ openFile: path, dockTab: "editor", dockOpen: true }),
  setPaneDrag: (on) =>
    set(on ? { paneDragging: true } : { paneDragging: false, paneDragOverWs: null }),
  setPaneDragOverWs: (id) => set({ paneDragOverWs: id }),
}));
