// The desktop's design tokens (src/styles/tokens.css), ported to RN. Same neutral near-black + one
// indigo accent, same spacing/type scale — so the phone reads as the same app.
//
// The accent is the desktop's oklch(0.488 0.217 264) converted to sRGB; RN has no oklch.
import { Platform } from "react-native";

export const c = {
  bg: "#161616",
  s1: "#161616", // chrome sits flat on the bg, split only by borders
  s2: "#1e1e1e", // cards, inputs, raised fills
  s3: "#282828", // pressed / active

  accent: "#1b4ed8",
  accentHover: "#2e64ea",
  accentDim: "rgba(255,255,255,0.07)", // hover/press fills stay neutral, not colored
  onAccent: "#ffffff",

  text1: "#f5f5f5",
  text2: "#a1a1a1",
  text3: "#767676",

  border1: "rgba(255,255,255,0.06)",
  border2: "rgba(255,255,255,0.10)",

  idle: "#737373",
  busy: "#f59e0b",
  awaiting: "#3b82f6",
  ok: "#10b981",
  error: "#ef4444",
  claim: "#a855f7",
} as const;

export const r = { one: 6, two: 10, three: 14 } as const;

export const sp = { 1: 4, 2: 8, 3: 12, 4: 16, 5: 24, 6: 32 } as const;

export const t = {
  xs: 11,
  sm: 12,
  base: 13,
  md: 14,
  lg: 16,
  xl: 21,
  xxl: 28,
} as const;

/** Deliberately the platform's own faces — no bundled webfonts to fail to load or bloat the APK. */
export const font = {
  ui: Platform.select({ android: "sans-serif", default: "System" }),
  uiMedium: Platform.select({ android: "sans-serif-medium", default: "System" }),
  mono: Platform.select({ android: "monospace", ios: "Menlo", default: "monospace" }),
} as const;

/** the pane/agent state colors, matching the desktop's status dots */
export const stateColor: Record<string, string> = {
  working: c.busy,
  waiting: c.awaiting,
  done: c.ok,
  idle: c.idle,
};

export const stateLabel: Record<string, string> = {
  working: "working",
  waiting: "needs you",
  done: "done",
  idle: "idle",
};

/** provider → the short tag shown on a pane row */
export const providerLabel: Record<string, string> = {
  claude: "Claude",
  gemini: "Gemini",
  codex: "Codex",
  opencode: "OpenCode",
  grok: "Grok",
  wsl: "WSL",
  terminal: "Shell",
};
