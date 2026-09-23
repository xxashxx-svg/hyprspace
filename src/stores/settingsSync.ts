import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useSettings, type CursorStyle, type ClaudePermission, type CodexMode, type UiFont, type DiffColors, type OpenTarget } from "./settings";
import type { Scheme } from "../themes";
import { saveState } from "../api";

type Snap = {
  theme: string;
  colorScheme: Scheme;
  uiFont: UiFont;
  diffColors: DiffColors;
  animations: boolean;
  fontSize: number;
  fontFamily: string;
  cursorStyle: CursorStyle;
  cursorBlink: boolean;
  copyOnSelect: boolean;
  lineHeight: number;
  terminalTheme: string;
  gpuRender: boolean;
  analytics: boolean;
  claudePermission: ClaudePermission;
  geminiYolo: boolean;
  codexMode: CodexMode;
  autoNameAgents: boolean;
  projectsDir: string;
  dismissedConfirms: string[];
  agentModel: Record<string, string>;
  agentEffort: Record<string, string>;
  lastProvider: string;
  editor: OpenTarget;
  railWidth: number;
  dockWidth: number;
  onboarded: boolean;
};

function snapshot(): Snap {
  const s = useSettings.getState();
  return {
    theme: s.theme,
    colorScheme: s.colorScheme,
    uiFont: s.uiFont,
    diffColors: s.diffColors,
    animations: s.animations,
    fontSize: s.fontSize,
    fontFamily: s.fontFamily,
    cursorStyle: s.cursorStyle,
    cursorBlink: s.cursorBlink,
    copyOnSelect: s.copyOnSelect,
    lineHeight: s.lineHeight,
    terminalTheme: s.terminalTheme,
    gpuRender: s.gpuRender,
    analytics: s.analytics,
    claudePermission: s.claudePermission,
    geminiYolo: s.geminiYolo,
    codexMode: s.codexMode,
    autoNameAgents: s.autoNameAgents,
    projectsDir: s.projectsDir,
    dismissedConfirms: s.dismissedConfirms,
    agentModel: s.agentModel,
    agentEffort: s.agentEffort,
    lastProvider: s.lastProvider,
    editor: s.editor,
    railWidth: s.railWidth,
    dockWidth: s.dockWidth,
    onboarded: s.onboarded,
  };
}

// Keep settings in lockstep between the main window and the settings window — they're separate
// webviews with separate stores. A change here persists to disk and broadcasts; a change from the
// other window applies locally (theme re-applies, terminals react) without echoing back.
// Call once per window AFTER the store has hydrated, so we don't broadcast the boot defaults.
export function initSettingsSync(): () => void {
  const me = getCurrentWindow().label;
  let lastSig = JSON.stringify(snapshot());
  let applyingRemote = false;

  // debounce disk writes + broadcasts — ctrl+scroll font zoom fires a change per wheel tick,
  // and each one used to be a full crash-safe JSON write
  let flushTimer: ReturnType<typeof setTimeout> | undefined;
  const unsub = useSettings.subscribe((s) => {
    if (!s.hydrated || applyingRemote) return;
    const sig = JSON.stringify(snapshot());
    if (sig === lastSig) return;
    lastSig = sig;
    clearTimeout(flushTimer);
    flushTimer = setTimeout(() => {
      const latest = JSON.stringify(snapshot());
      lastSig = latest;
      void saveState("settings", latest).catch((e) => console.error("settings save failed:", e));
      void emit("settings:changed", { source: me, snap: JSON.parse(latest) as Snap });
    }, 300);
  });

  const unlistenP = listen<{ source: string; snap: Snap }>("settings:changed", (e) => {
    if (e.payload.source === me) return; // our own broadcast bouncing back
    applyingRemote = true;
    try {
      useSettings.getState().hydrate(e.payload.snap); // sets values + re-applies the theme
      lastSig = JSON.stringify(snapshot());
    } finally {
      applyingRemote = false;
    }
  });

  return () => {
    unsub();
    void unlistenP.then((u) => u());
  };
}
