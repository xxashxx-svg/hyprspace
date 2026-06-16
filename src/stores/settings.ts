import { create } from "zustand";
import { applyTheme } from "../themes";

export type CursorStyle = "bar" | "block" | "underline";

interface SettingsState {
  theme: string;
  fontSize: number;
  fontFamily: string;
  cursorStyle: CursorStyle;
  cursorBlink: boolean;
  copyOnSelect: boolean;
  hydrated: boolean;
  setTheme: (id: string) => void;
  setFontSize: (n: number) => void;
  setFontFamily: (f: string) => void;
  setCursorStyle: (c: CursorStyle) => void;
  setCursorBlink: (b: boolean) => void;
  setCopyOnSelect: (b: boolean) => void;
  hydrate: (partial: Partial<SettingsState>) => void;
  markHydrated: () => void;
}

export const DEFAULT_FONT = '"Cascadia Code", "JetBrains Mono", "Consolas", monospace';

export const useSettings = create<SettingsState>()((set) => ({
  theme: "ember",
  fontSize: 13,
  fontFamily: DEFAULT_FONT,
  cursorStyle: "bar",
  cursorBlink: true,
  copyOnSelect: false,
  hydrated: false,

  setTheme: (id) => {
    applyTheme(id);
    set({ theme: id });
  },
  setFontSize: (n) => set({ fontSize: Math.min(24, Math.max(9, Math.round(n))) }),
  setFontFamily: (f) => set({ fontFamily: f }),
  setCursorStyle: (c) => set({ cursorStyle: c }),
  setCursorBlink: (b) => set({ cursorBlink: b }),
  setCopyOnSelect: (b) => set({ copyOnSelect: b }),

  hydrate: (partial) => {
    set({ ...partial, hydrated: true });
    applyTheme(useSettings.getState().theme);
  },
  markHydrated: () => set({ hydrated: true }),
}));
