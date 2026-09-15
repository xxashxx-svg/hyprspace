import { Terminal, type ITheme } from "@xterm/xterm";
import { WebglAddon } from "@xterm/addon-webgl";
import "@xterm/xterm/css/xterm.css";
import { useSettings } from "../stores/settings";
import { paletteById } from "./palettes";

// terminal palette: on "adaptive" (default) bg/fg/cursor/selection come from the active theme's CSS
// vars (so the bg blends with the app surface, T3-style) and the ANSI 16 are T3 Code's muted set;
// pick any named scheme in Settings → Terminal and we hand xterm that palette's colors instead.
export function termTheme(): ITheme {
  const p = paletteById(useSettings.getState().terminalTheme);
  if (!p.adaptive && p.theme) return p.theme;
  const css = getComputedStyle(document.documentElement);
  const v = (n: string, fb: string) => css.getPropertyValue(n).trim() || fb;
  const bg = v("--bg-terminal", "#161616");
  const base = {
    background: bg,
    foreground: v("--term-fg", "rgb(237, 241, 247)"),
    cursor: v("--term-cursor", "rgb(180, 203, 255)"),
    cursorAccent: bg,
    selectionBackground: v("--term-selection", "rgba(180, 203, 255, 0.25)"),
  };
  // the light side: the same 16 slots, deep enough to read on a near-white page
  if (document.documentElement.dataset.scheme === "light") {
    return {
      ...base,
      black: "rgb(40, 44, 52)",
      red: "rgb(196, 40, 60)",
      green: "rgb(36, 132, 68)",
      yellow: "rgb(170, 116, 0)",
      blue: "rgb(30, 100, 214)",
      magenta: "rgb(146, 60, 186)",
      cyan: "rgb(0, 134, 154)",
      white: "rgb(120, 128, 140)",
      brightBlack: "rgb(110, 118, 130)",
      brightRed: "rgb(216, 60, 80)",
      brightGreen: "rgb(46, 152, 84)",
      brightYellow: "rgb(186, 132, 10)",
      brightBlue: "rgb(50, 120, 236)",
      brightMagenta: "rgb(166, 80, 206)",
      brightCyan: "rgb(10, 154, 176)",
      brightWhite: "rgb(30, 34, 40)",
    };
  }
  return {
    ...base,
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

// Every terminal that shares a font, size, theme and DPR SHARES ONE glyph atlas — xterm caches them
// by config (@xterm/addon-webgl CharAtlasCache), and all our panes match. Clearing the atlas from
// one pane repacks that shared texture but only resets THAT pane's cell cache. Every other pane
// keeps texture coordinates pointing into the old packing, and since the renderer redraws only
// cells whose content changed, any cell that never changes keeps drawing from a slot that now holds
// a different glyph. That is the character stuck beside claude's prompt: the space next to the
// marker is the one cell on a busy line that never changes, so it is the one that rots.
// So an atlas clear has to be an all-panes repaint, never a per-pane one.
// Only panes holding a GPU renderer belong here. A pane without one has no atlas to go stale, and a
// pane that gave its renderer up rebuilds model, vertices and all when it gets a new one.
const live = new Set<Terminal>();

export function trackTerminal(term: Terminal): void {
  live.add(term);
}

export function untrackTerminal(term: Terminal): void {
  live.delete(term);
}

/**
 * Invalidate the shared glyph atlas and redraw every live pane from scratch. Coalesced to one
 * repaint per frame — every mounted pane listens for the same window focus and theme change, so
 * without this a grid of twelve would run this twelve times over.
 */
let queued = false;
export function repaintAllTerminals(): void {
  if (queued) return;
  queued = true;
  requestAnimationFrame(() => {
    queued = false;
    // clear first, everywhere: only the first call actually repacks the atlas, but each one resets
    // that pane's cell cache, which is what stops it drawing from the old packing
    for (const t of live) {
      try {
        t.clearTextureAtlas?.();
      } catch {
        /* renderer not ready */
      }
    }
    for (const t of live) {
      try {
        t.refresh(0, t.rows - 1);
      } catch {
        /* renderer not ready */
      }
    }
  });
}

// Attach the GPU (WebGL) renderer when it's enabled — same atlas-and-quads model as Alacritty's
// OpenGL renderer, so block art (the Claude logo, progress bars, box-drawing) tiles seamlessly.
// MUST be called after term.open(). Returns the addon so callers can detach it again — panes drop
// GPU while their space is hidden, since browsers cap WebGL contexts (~16 per page) and every
// space's panes stay mounted. Null on the DOM setting or if WebGL is unavailable.
// On context loss the addon self-disposes (reverting xterm to the DOM renderer). `onLoss` runs
// FIRST — the addon's dispose tears down its emitter mid-fire, so a callback registered after the
// dispose handler would never run.
export function attachGpuRenderer(term: Terminal, onLoss?: () => void): WebglAddon | null {
  if (!useSettings.getState().gpuRender) return null;
  try {
    const webgl = new WebglAddon();
    webgl.onContextLoss(() => {
      onLoss?.();
      webgl.dispose();
    });
    term.loadAddon(webgl);
    return webgl;
  } catch (err) {
    console.warn("WebGL renderer unavailable; staying on the DOM renderer", err);
    return null;
  }
}
