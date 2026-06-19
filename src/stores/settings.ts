import { create } from "zustand";
import { applyTheme } from "../themes";

export type CursorStyle = "bar" | "block" | "underline";
export type ClaudePermission = "default" | "acceptEdits" | "plan" | "bypass";
export type CodexMode = "default" | "auto" | "bypass";

interface SettingsState {
  theme: string;
  fontSize: number;
  fontFamily: string;
  cursorStyle: CursorStyle;
  cursorBlink: boolean;
  copyOnSelect: boolean;
  claudePermission: ClaudePermission;
  geminiYolo: boolean;
  codexMode: CodexMode;
  projectsDir: string; // base folder for new projects; "" → ~/Documents/HyprSpace
  hydrated: boolean;
  setTheme: (id: string) => void;
  setFontSize: (n: number) => void;
  setFontFamily: (f: string) => void;
  setCursorStyle: (c: CursorStyle) => void;
  setCursorBlink: (b: boolean) => void;
  setCopyOnSelect: (b: boolean) => void;
  setClaudePermission: (m: ClaudePermission) => void;
  setGeminiYolo: (b: boolean) => void;
  setCodexMode: (m: CodexMode) => void;
  setProjectsDir: (p: string) => void;
  hydrate: (partial: Partial<SettingsState>) => void;
  markHydrated: () => void;
}

export const DEFAULT_FONT = '"Cascadia Code", "JetBrains Mono", "Consolas", monospace';

export const useSettings = create<SettingsState>()((set) => ({
  theme: "t3",
  fontSize: 13,
  fontFamily: DEFAULT_FONT,
  cursorStyle: "bar",
  cursorBlink: true,
  copyOnSelect: false,
  claudePermission: "acceptEdits",
  geminiYolo: false,
  codexMode: "auto",
  projectsDir: "",
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
  setClaudePermission: (m) => set({ claudePermission: m }),
  setGeminiYolo: (b) => set({ geminiYolo: b }),
  setCodexMode: (m) => set({ codexMode: m }),
  setProjectsDir: (p) => set({ projectsDir: p.trim() }),

  hydrate: (partial) => {
    set({ ...partial, hydrated: true });
    applyTheme(useSettings.getState().theme);
  },
  markHydrated: () => set({ hydrated: true }),
}));
