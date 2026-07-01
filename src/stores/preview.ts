import { create } from "zustand";

// The embedded browser preview (a docked iframe). Actions with a preview URL open it here instead of
// kicking you out to a real browser. A tick bumps to force a reload of the same URL.
interface PreviewState {
  open: boolean;
  url: string;
  tick: number;
  openUrl: (url: string) => void;
  setUrl: (url: string) => void;
  reload: () => void;
  close: () => void;
  toggle: () => void;
}

export const usePreview = create<PreviewState>((set, get) => ({
  open: false,
  url: "",
  tick: 0,
  // named `open` on the store call site reads oddly with the boolean; expose openUrl
  openUrl: (url) => set({ open: true, url, tick: get().tick + 1 }),
  setUrl: (url) => set({ url }),
  reload: () => set((s) => ({ tick: s.tick + 1 })),
  close: () => set({ open: false }),
  toggle: () => set((s) => ({ open: !s.open })),
}));
