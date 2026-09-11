import { create } from "zustand";

interface UiState {
  view: "home" | "space" | "launch"; // home composer, a workspace, or the multi-agent launcher
  launchReturn: "home" | "space"; // where the launcher's Cancel/Esc sends you back to
  railCollapsed: boolean;
  // Which pane is fullscreen, PER PROJECT. A single global id meant maximizing in one project
  // silently wiped the other's — you'd come back and find it tiled again.
  maximizedByWs: Record<string, string>; // wsId → session id shown fullscreen there
  fileDropId: string | null; // session id of the pane a file drag is currently over
  skillDropId: string | null; // pane a skill chip is being dragged over
  settingsOpen: boolean; // in-app settings screen
  signInOpen: boolean; // the sign-in screen, opened on purpose — an account is never required
  settingsTab: string;
  newProjectOpen: boolean; // the New Project wizard
  paletteOpen: boolean;
  dockOpen: boolean;
  dockTab: "files" | "git";
  paneDragging: boolean; // a terminal pane is mid-drag (rail shows spaces as drop targets)
  paneDragOverWs: string | null; // the space the dragged pane is hovering over in the rail
  goHome: () => void;
  goSpace: () => void;
  toggleRail: () => void;
  toggleMaximized: (wsId: string, id: string) => void;
  clearMaximized: (wsId: string) => void;
  setFileDrop: (id: string | null) => void;
  setSkillDrop: (id: string | null) => void;
  openSettings: (tab?: string) => void;
  closeSettings: () => void;
  openSignIn: () => void;
  closeSignIn: () => void;
  setSettingsTab: (t: string) => void;
  openNewProject: () => void;
  closeNewProject: () => void;
  openLaunch: () => void;
  closeLaunch: () => void;
  togglePalette: () => void;
  setPalette: (b: boolean) => void;
  toggleDock: () => void;
  setDock: (b: boolean) => void;
  setDockTab: (t: "files" | "git") => void;
  onboardingOpen: boolean;
  openOnboarding: () => void;
  closeOnboarding: () => void;
  setPaneDrag: (on: boolean) => void;
  setPaneDragOverWs: (id: string | null) => void;
}

export const useUi = create<UiState>()((set) => ({
  view: "home", // land on the dashboard
  launchReturn: "home",
  railCollapsed: false,
  maximizedByWs: {},
  fileDropId: null,
  skillDropId: null,
  settingsOpen: false,
  signInOpen: false,
  settingsTab: "general",
  newProjectOpen: false,
  paletteOpen: false,
  dockOpen: false,
  dockTab: "files",
  paneDragging: false,
  paneDragOverWs: null,
  goHome: () => set({ view: "home" }),
  goSpace: () => set({ view: "space" }),
  toggleRail: () => set((s) => ({ railCollapsed: !s.railCollapsed })),
  toggleMaximized: (wsId, id) =>
    set((s) => {
      const next = { ...s.maximizedByWs };
      if (next[wsId] === id) delete next[wsId];
      else next[wsId] = id;
      return { maximizedByWs: next };
    }),
  clearMaximized: (wsId) =>
    set((s) => {
      if (!(wsId in s.maximizedByWs)) return {};
      const next = { ...s.maximizedByWs };
      delete next[wsId];
      return { maximizedByWs: next };
    }),
  setFileDrop: (id) => set({ fileDropId: id }),
  setSkillDrop: (id) => set({ skillDropId: id }),
  openSettings: (tab) => set((s) => ({ settingsOpen: true, settingsTab: tab ?? s.settingsTab })),
  closeSettings: () => set({ settingsOpen: false }),
  openSignIn: () => set({ signInOpen: true }),
  closeSignIn: () => set({ signInOpen: false }),
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
