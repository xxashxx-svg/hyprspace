import { create } from "zustand";
import { applyTheme, THEMES, type Mode, type Scheme } from "../themes";
import type { EditorId } from "../api";

/** What the titlebar's Open button opens a folder in: an editor, or the OS file manager */
export type OpenTarget = EditorId | "files";

export type CursorStyle = "bar" | "block" | "underline";
export type ClaudePermission = "default" | "acceptEdits" | "plan" | "bypass";
export type CodexMode = "default" | "auto" | "bypass";
export type UiFont = "dm" | "system";
export type DiffColors = "redgreen" | "blueorange";

export const UI_FONTS: Record<UiFont, { label: string; stack: string }> = {
  dm: { label: "DM Sans (bundled)", stack: '"DM Sans Variable", "DM Sans", system-ui, -apple-system, "Segoe UI", "Cantarell", sans-serif' },
  system: { label: "System", stack: 'system-ui, -apple-system, "Segoe UI", "Cantarell", sans-serif' },
};

interface SettingsState {
  theme: string;
  colorScheme: Scheme; // light, dark, or follow the OS
  mode: Mode; // which side colorScheme resolved to. Derived on paint, never persisted.
  uiFont: UiFont; // everything outside the terminal
  diffColors: DiffColors; // additions and deletions: red and green, or blue and orange
  animations: boolean; // off = menus, panels and dialogs snap instead of easing
  fontSize: number;
  fontFamily: string;
  cursorStyle: CursorStyle;
  cursorBlink: boolean;
  copyOnSelect: boolean;
  lineHeight: number; // terminal row spacing (1.0 = tight, 1.2 = airy)
  terminalTheme: string; // terminal color scheme id (see terminal/palettes.ts); "adaptive" = match app
  gpuRender: boolean; // WebGL renderer (GPU, seamless block art) vs the DOM renderer (ClearType text)
  analytics: boolean; // one anonymous app_opened ping (install id + version + OS). See lib/analytics.ts.
  claudePermission: ClaudePermission;
  geminiYolo: boolean;
  codexMode: CodexMode;
  autoNameAgents: boolean; // task-name agent panes via Codex (kill switch for the auto-namer)
  projectsDir: string; // base folder for new projects; "" → ~/Documents/HyprSpace
  onboarded: boolean; // first-run wizard done (or skipped) — existing users get it set silently
  dismissedConfirms: string[]; // "don't ask again" ids
  agentModel: Record<string, string>; // provider id → model id ("" = the CLI's default)
  agentEffort: Record<string, string>; // provider id → effort level ("" = the CLI's default)
  lastProvider: string; // what the composer launched last; the next composer opens on it
  editor: OpenTarget; // what the titlebar's Open button opens a folder in
  railWidth: number; // left sidebar width in px
  dockWidth: number; // right dock width in px
  hydrated: boolean;
  setTheme: (id: string) => void;
  setColorScheme: (s: Scheme) => void;
  setUiFont: (f: UiFont) => void;
  setDiffColors: (d: DiffColors) => void;
  setAnimations: (b: boolean) => void;
  /** apply theme, scheme, font, diff colors and motion to the document */
  repaint: () => void;
  setFontSize: (n: number) => void;
  setFontFamily: (f: string) => void;
  setCursorStyle: (c: CursorStyle) => void;
  setCursorBlink: (b: boolean) => void;
  setCopyOnSelect: (b: boolean) => void;
  setLineHeight: (n: number) => void;
  setTerminalTheme: (id: string) => void;
  setGpuRender: (b: boolean) => void;
  setAnalytics: (b: boolean) => void;
  setClaudePermission: (m: ClaudePermission) => void;
  setGeminiYolo: (b: boolean) => void;
  setCodexMode: (m: CodexMode) => void;
  setAutoNameAgents: (b: boolean) => void;
  setProjectsDir: (p: string) => void;
  setAgentModel: (provider: string, model: string) => void;
  setAgentEffort: (provider: string, effort: string) => void;
  setLastProvider: (id: string) => void;
  setEditor: (id: OpenTarget) => void;
  setRailWidth: (n: number) => void;
  setDockWidth: (n: number) => void;
  setOnboarded: (b: boolean) => void;
  dismissConfirm: (id: string) => void;
  resetDismissedConfirms: () => void;
  hydrate: (partial: Partial<SettingsState>) => void;
  markHydrated: () => void;
}

// bundled Nerd Font first, then per-OS fallbacks: Cascadia/Consolas (Windows), Menlo (mac),
// DejaVu/Liberation (Linux — one of the two ships on essentially every distro)
export const DEFAULT_FONT =
  '"JetBrainsMono Nerd Font", "JetBrains Mono", "Cascadia Code", "Consolas", "Menlo", "DejaVu Sans Mono", "Liberation Mono", monospace';

export const useSettings = create<SettingsState>()((set, get) => ({
  theme: "t3",
  colorScheme: "dark",
  mode: "dark",
  uiFont: "dm",
  diffColors: "redgreen",
  animations: true,
  fontSize: 13,
  fontFamily: DEFAULT_FONT,
  cursorStyle: "block",
  cursorBlink: true,
  copyOnSelect: false,
  lineHeight: 1.1, // comfortable middle — 1.0 felt congested, 1.2 felt airy
  terminalTheme: "adaptive", // follow the app theme by default; pick a named scheme in Settings
  analytics: true, // on by default, off in one click — a count of installs nobody can trace to a person
  gpuRender: true, // GPU/WebGL by default — block art (Claude logo, progress bars) tiles seamlessly
  // at any line height; the DOM/ClearType renderer is the opt-out for folks who prefer subpixel text
  claudePermission: "acceptEdits",
  geminiYolo: false,
  codexMode: "auto",
  autoNameAgents: false, // off by default — opt in via Settings (uses your Codex free quota)
  projectsDir: "",
  dismissedConfirms: [],
  agentModel: {},
  agentEffort: {},
  lastProvider: "claude",
  editor: "code",
  railWidth: 272,
  dockWidth: 380,
  onboarded: false,
  hydrated: false,

  setTheme: (id) => {
    set({ theme: id });
    get().repaint();
  },
  setColorScheme: (colorScheme) => {
    set({ colorScheme });
    get().repaint();
  },
  setUiFont: (uiFont) => {
    set({ uiFont });
    get().repaint();
  },
  setDiffColors: (diffColors) => {
    set({ diffColors });
    get().repaint();
  },
  setAnimations: (animations) => {
    set({ animations });
    get().repaint();
  },
  repaint: () => {
    const s = get();
    const root = document.documentElement;
    root.style.setProperty("--font-ui", UI_FONTS[s.uiFont].stack);
    root.dataset.diff = s.diffColors;
    root.classList.toggle("reduce-motion", !s.animations);
    set({ mode: applyTheme(s.theme, s.colorScheme) });
  },
  setFontSize: (n) => set({ fontSize: Math.min(24, Math.max(9, Math.round(n))) }),
  setFontFamily: (f) => set({ fontFamily: f }),
  setCursorStyle: (c) => set({ cursorStyle: c }),
  setCursorBlink: (b) => set({ cursorBlink: b }),
  setCopyOnSelect: (b) => set({ copyOnSelect: b }),
  setLineHeight: (n) => set({ lineHeight: Math.min(1.8, Math.max(1.0, Math.round(n * 100) / 100)) }),
  setTerminalTheme: (id) => set({ terminalTheme: id }),
  setGpuRender: (b) => set({ gpuRender: b }),
  setAnalytics: (b) => set({ analytics: b }),
  setClaudePermission: (m) => set({ claudePermission: m }),
  setGeminiYolo: (b) => set({ geminiYolo: b }),
  setCodexMode: (m) => set({ codexMode: m }),
  setAutoNameAgents: (b) => set({ autoNameAgents: b }),
  setProjectsDir: (p) => set({ projectsDir: p.trim() }),
  setAgentModel: (provider, model) => set((s) => ({ agentModel: { ...s.agentModel, [provider]: model } })),
  setAgentEffort: (provider, effort) => set((s) => ({ agentEffort: { ...s.agentEffort, [provider]: effort } })),
  setLastProvider: (id) => set({ lastProvider: id }),
  setEditor: (id) => set({ editor: id }),
  setRailWidth: (n) => set({ railWidth: Math.min(520, Math.max(200, Math.round(n))) }),
  setDockWidth: (n) => set({ dockWidth: Math.min(720, Math.max(260, Math.round(n))) }),
  setOnboarded: (b) => set({ onboarded: b }),
  dismissConfirm: (id) =>
    set((s) => (s.dismissedConfirms.includes(id) ? {} : { dismissedConfirms: [...s.dismissedConfirms, id] })),
  resetDismissedConfirms: () => set({ dismissedConfirms: [] }),

  hydrate: (partial) => {
    // a theme id from an older release falls back to the default rather than lingering unmatched
    const theme = partial.theme && THEMES.some((t) => t.id === partial.theme) ? partial.theme : "t3";
    set({ ...partial, theme, hydrated: true });
    get().repaint();
  },
  markHydrated: () => set({ hydrated: true }),
}));

// following the OS: repaint when it flips between light and dark
if (typeof window !== "undefined") {
  window.matchMedia?.("(prefers-color-scheme: light)").addEventListener("change", () => {
    if (useSettings.getState().colorScheme === "system") useSettings.getState().repaint();
  });
}
