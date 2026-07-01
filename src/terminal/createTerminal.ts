import { Terminal, type ITheme } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { useSettings } from "../stores/settings";

// terminal palette: bg/fg/cursor/selection come from the active theme's CSS vars (so the bg blends
// with the app surface, T3-style); the ANSI 16 are T3 Code's muted set so output looks soft and
// cohesive instead of harsh-default.
export function termTheme(): ITheme {
  const css = getComputedStyle(document.documentElement);
  const v = (n: string, fb: string) => css.getPropertyValue(n).trim() || fb;
  const bg = v("--bg-terminal", "#161616");
  return {
    background: bg,
    foreground: v("--term-fg", "rgb(237, 241, 247)"),
    cursor: v("--term-cursor", "rgb(180, 203, 255)"),
    cursorAccent: bg,
    selectionBackground: v("--term-selection", "rgba(180, 203, 255, 0.25)"),
    black: "rgb(24, 30, 38)",
    red: "rgb(255, 122, 142)",
    green: "rgb(134, 231, 149)",
    yellow: "rgb(244, 205, 114)",
    blue: "rgb(137, 190, 255)",
    magenta: "rgb(208, 176, 255)",
    cyan: "rgb(124, 232, 237)",
    white: "rgb(210, 218, 230)",
    brightBlack: "rgb(110, 120, 136)",
    brightRed: "rgb(255, 168, 180)",
    brightGreen: "rgb(176, 245, 186)",
    brightYellow: "rgb(255, 224, 149)",
    brightBlue: "rgb(174, 210, 255)",
    brightMagenta: "rgb(229, 203, 255)",
    brightCyan: "rgb(167, 244, 247)",
    brightWhite: "rgb(244, 247, 252)",
  };
}

// Claude draws its own block cursor; for shell panes we honor the user's cursor settings.
export function makeTerminal(isClaude: boolean): Terminal {
  const s = useSettings.getState();
  // On Windows the PTY backend is ConPTY; declaring it lets xterm reconstruct wrapped
  // lines so scrollback can reflow on resize instead of staying stuck at the old width.
  const windowsPty = navigator.userAgent.includes("Windows")
    ? ({ backend: "conpty" } as const)
    : undefined;
  return new Terminal({
    cursorStyle: s.cursorStyle,
    cursorInactiveStyle: "none",
    cursorBlink: isClaude ? false : s.cursorBlink,
    fontFamily: s.fontFamily,
    fontSize: s.fontSize,
    // 1.0 so block-element glyphs (the Claude logo, box-art, progress bars) tile seamlessly —
    // any line-height > 1 leaves a gap above each row and the filled art looks striped/broken.
    // 1.0 also sidesteps the DOM renderer's fractional-line-height last-row clipping.
    lineHeight: 1.0,
    scrollback: 10000,
    smoothScrollDuration: 80,
    fastScrollSensitivity: 5,
    rescaleOverlappingGlyphs: true, // crisper box-drawing / powerline glyphs
    allowProposedApi: true,
    windowsPty,
    theme: termTheme(),
  });
}
