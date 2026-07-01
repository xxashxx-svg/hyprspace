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
  lineHeight: number; // terminal row spacing (1.0 = tight, 1.2 = airy)
  gpuRender: boolean; // WebGL renderer (GPU, seamless block art) vs the DOM renderer (ClearType text)
  claudePermission: ClaudePermission;
  geminiYolo: boolean;
  codexMode: CodexMode;
  autoNameAgents: boolean; // task-name agent panes via Codex (kill switch for the auto-namer)
  projectsDir: string; // base folder for new projects; "" → ~/Documents/HyprSpace
  hydrated: boolean;
  setTheme: (id: string) => void;
  setFontSize: (n: number) => void;
  setFontFamily: (f: string) => void;
  setCursorStyle: (c: CursorStyle) => void;
  setCursorBlink: (b: boolean) => void;
  setCopyOnSelect: (b: boolean) => void;
  setLineHeight: (n: number) => void;
  setGpuRender: (b: boolean) => void;
  setClaudePermission: (m: ClaudePermission) => void;
  setGeminiYolo: (b: boolean) => void;
  setCodexMode: (m: CodexMode) => void;
  setAutoNameAgents: (b: boolean) => void;
  setProjectsDir: (p: string) => void;
  hydrate: (partial: Partial<SettingsState>) => void;
  markHydrated: () => void;
}

export const DEFAULT_FONT = '"Cascadia Code", "JetBrains Mono", "Consolas", monospace';

export const useSettings = create<SettingsState>()((set) => ({
  theme: "t3",
  fontSize: 13,
  fontFamily: '"JetBrains Mono", "Cascadia Code", monospace',
  cursorStyle: "block",
  cursorBlink: true,
  copyOnSelect: false,
  lineHeight: 1.1, // comfortable middle — 1.0 felt congested, 1.2 felt airy
  gpuRender: false, // DOM renderer by default = crisp ClearType text (WebGL is softer; opt in for it)
  claudePermission: "acceptEdits",
  geminiYolo: false,
  codexMode: "auto",
  autoNameAgents: false, // off by default — opt in via Settings (uses your Codex free quota)
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
  setLineHeight: (n) => set({ lineHeight: Math.min(1.8, Math.max(1.0, Math.round(n * 100) / 100)) }),
  setGpuRender: (b) => set({ gpuRender: b }),
  setClaudePermission: (m) => set({ claudePermission: m }),
  setGeminiYolo: (b) => set({ geminiYolo: b }),
  setCodexMode: (m) => set({ codexMode: m }),
  setAutoNameAgents: (b) => set({ autoNameAgents: b }),
  setProjectsDir: (p) => set({ projectsDir: p.trim() }),

  hydrate: (partial) => {
    set({ ...partial, hydrated: true });
    applyTheme(useSettings.getState().theme);
  },
  markHydrated: () => set({ hydrated: true }),
}));
