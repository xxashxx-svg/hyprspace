import { Terminal, type ITheme } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { useSettings } from "../stores/settings";

// terminal palette: bg/fg/cursor/selection come from the active theme's CSS vars;
// the ANSI 16 are a shared, dark-tuned set so output colors stay consistent across themes.
export function termTheme(): ITheme {
  const css = getComputedStyle(document.documentElement);
  const v = (n: string, fb: string) => css.getPropertyValue(n).trim() || fb;
  const bg = v("--bg-terminal", "#141318");
  return {
    background: bg,
    foreground: v("--term-fg", "#e7e9ee"),
    cursor: v("--term-cursor", "#a78bfa"),
    cursorAccent: bg,
    selectionBackground: v("--term-selection", "rgba(139,92,246,0.30)"),
    black: "#1a1a2e",
    red: "#f0708b",
    green: "#8be9a1",
    yellow: "#f5d88e",
    blue: "#7dc4e8",
    magenta: "#c49cf0",
    cyan: "#7ed3c7",
    white: "#d4daf0",
    brightBlack: "#4a5068",
    brightRed: "#f5899e",
    brightGreen: "#a3f0b5",
    brightYellow: "#f8e5a0",
    brightBlue: "#96d0ef",
    brightMagenta: "#d4b4f5",
    brightCyan: "#96e0d6",
    brightWhite: "#eef0f8",
  };
}

// Claude draws its own block cursor; for shell panes we honor the user's cursor settings.
export function makeTerminal(isClaude: boolean): Terminal {
  const s = useSettings.getState();
  return new Terminal({
    cursorStyle: s.cursorStyle,
    cursorInactiveStyle: "none",
    cursorBlink: isClaude ? false : s.cursorBlink,
    fontFamily: s.fontFamily,
    fontSize: s.fontSize,
    lineHeight: 1.2,
    scrollback: 10000,
    smoothScrollDuration: 80,
    allowProposedApi: true,
    theme: termTheme(),
  });
}
