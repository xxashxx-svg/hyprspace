import { Terminal, type ITheme } from "@xterm/xterm";
import { WebglAddon } from "@xterm/addon-webgl";
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
    // user-tunable (Settings → Terminal). Note: on the DOM renderer, > 1.0 gives filled block-art
    // (logo/progress bars) a faint per-row gap; WebGL draws blocks as quads so it's seamless anywhere.
    lineHeight: s.lineHeight ?? 1.1,
    scrollback: 10000,
    smoothScrollDuration: 80,
    fastScrollSensitivity: 5,
    rescaleOverlappingGlyphs: true, // crisper box-drawing / powerline glyphs
    // draw block/box-drawing chars as filled rects instead of font glyphs (GPU renderer only) —
    // this is what makes the Claude logo one solid shape instead of striped rows
    customGlyphs: true,
    allowProposedApi: true,
    windowsPty,
    theme: termTheme(),
  });
}

// Attach the GPU (WebGL) renderer when it's enabled — same atlas-and-quads model as Alacritty's
// OpenGL renderer, so block art (the Claude logo, progress bars, box-drawing) tiles seamlessly.
// MUST be called after term.open(). Returns the addon so callers can detach it again — panes drop
// GPU while their space is hidden, since browsers cap WebGL contexts (~16 per page) and every
// space's panes stay mounted. Null on the DOM setting or if WebGL is unavailable; on context loss
// it self-disposes, reverting xterm to the DOM renderer instead of blanking.
export function attachGpuRenderer(term: Terminal): WebglAddon | null {
  if (!useSettings.getState().gpuRender) return null;
  try {
    const webgl = new WebglAddon();
    webgl.onContextLoss(() => webgl.dispose());
    term.loadAddon(webgl);
    return webgl;
  } catch (err) {
    console.warn("WebGL renderer unavailable; staying on the DOM renderer", err);
    return null;
  }
}
