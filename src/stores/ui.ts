import { create } from "zustand";

interface UiState {
  view: "home" | "space" | "loops" | "launch"; // home dashboard, a workspace, the loops page, or the multi-agent launcher
  launchReturn: "home" | "space" | "loops"; // where the launcher's Cancel/Esc sends you back to
  openLoopId: string | null; // the loop selected in the Loops page master-detail
  loopsTab: "runs" | "manage"; // the live runs view vs the classic config manager
  railCollapsed: boolean;
  maximizedId: string | null; // session id of the zoomed-to-fullscreen pane
  fileDropId: string | null; // session id of the pane a file drag is currently over
  skillDropId: string | null; // pane a skill chip is being dragged over
  settingsOpen: boolean; // in-app settings screen
  settingsTab: string;
  newProjectOpen: boolean; // the New Project wizard
  servicesFor: { folder: string; wsId: string; name: string } | null; // the Services config modal
  serviceLogsFor: { id: string; name: string } | null; // the background-service log viewer
  paletteOpen: boolean;
  dockOpen: boolean;
  dockTab: "changes" | "skills" | "services" | "files" | "editor";
  openFile: string | null; // absolute path open in the editor tab
  editorMax: boolean; // editor expanded over the whole workspace area
  paneDragging: boolean; // a terminal pane is mid-drag (rail shows spaces as drop targets)
  paneDragOverWs: string | null; // the space the dragged pane is hovering over in the rail
  goHome: () => void;
  goSpace: () => void;
  goLoops: () => void;
  focusLoop: (id: string) => void; // open a specific loop in the Loops page (runs view)
  setLoopsTab: (t: "runs" | "manage") => void;
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
  closeFile: () => void;
  toggleEditorMax: () => void;
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
  loopsTab: "runs",
  railCollapsed: false,
  maximizedId: null,
  fileDropId: null,
  skillDropId: null,
  settingsOpen: false,
  settingsTab: "appearance",
  newProjectOpen: false,
  servicesFor: null,
  serviceLogsFor: null,
  paletteOpen: false,
  dockOpen: false,
  dockTab: "skills",
  openFile: null,
  editorMax: false,
  paneDragging: false,
  paneDragOverWs: null,
  goHome: () => set({ view: "home" }),
  goSpace: () => set({ view: "space" }),
  goLoops: () => set({ view: "loops" }),
  focusLoop: (id) => set({ view: "loops", loopsTab: "runs", openLoopId: id }),
  setLoopsTab: (t) => set({ loopsTab: t }),
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
  // closing lands you back on the file tree, since the editor tab disappears with the file
  closeFile: () => set({ openFile: null, editorMax: false, dockTab: "files" }),
  toggleEditorMax: () => set((s) => ({ editorMax: !s.editorMax })),
  onboardingOpen: false,
  openOnboarding: () => set({ onboardingOpen: true, settingsOpen: false }),
  closeOnboarding: () => set({ onboardingOpen: false }),
  setPaneDrag: (on) =>
    set(on ? { paneDragging: true } : { paneDragging: false, paneDragOverWs: null }),
  setPaneDragOverWs: (id) => set({ paneDragOverWs: id }),
}));
