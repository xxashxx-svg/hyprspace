// Themes: a hue and a name. Every token for the light and the dark side is derived from that hue,
// so the six themes read as one family and adding one is a single line. Shared bits (status
// colors, radii, fonts) live in tokens.css.
export type Scheme = "system" | "light" | "dark";
export type Mode = "light" | "dark";

export interface Theme {
  id: string;
  name: string;
  blurb: string;
  vars: Record<Mode, Record<string, string>>;
}

// oklch keeps every hue at the same perceived lightness, so a tinted surface reads as "slightly
// warm" or "slightly cool" rather than one theme looking brighter than the next.
const ok = (l: number, c: number, h: number, a?: number) =>
  a == null ? `oklch(${l} ${c} ${h})` : `oklch(${l} ${c} ${h} / ${a})`;

interface Spec {
  hue: number;
  tint: number; // chroma of the surfaces: 0 = pure neutral, ~0.012 = a visible cast
  dark: { l: number; c: number }; // accent on the dark side
  light: { l: number; c: number }; // accent on the light side
}

function build(id: string, name: string, blurb: string, sp: Spec): Theme {
  const { hue, tint } = sp;
  // white text on a deep accent, near-black text on a bright one
  const onAccent = (l: number) => (l >= 0.66 ? ok(0.2, 0.02, hue) : "#ffffff");
  const dark: Record<string, string> = {
    "--bg-base": ok(0.2, tint, hue),
    "--surface-1": ok(0.2, tint, hue),
    "--surface-2": ok(0.24, tint, hue),
    "--surface-3": ok(0.29, tint, hue),
    "--bg-terminal": ok(0.2, tint, hue),
    "--accent": ok(sp.dark.l, sp.dark.c, hue),
    "--accent-hover": ok(sp.dark.l + 0.06, sp.dark.c, hue),
    "--accent-dim": "rgba(255, 255, 255, 0.07)",
    "--on-accent": onAccent(sp.dark.l),
    "--text-1": ok(0.97, tint / 2, hue),
    "--text-2": ok(0.72, tint / 2, hue),
    "--text-3": ok(0.56, tint / 2, hue),
    "--border-0": "rgba(255, 255, 255, 0.035)",
    "--border-1": "rgba(255, 255, 255, 0.06)",
    "--border-2": "rgba(255, 255, 255, 0.1)",
    "--ink": "255, 255, 255",
    "--term-fg": ok(0.95, tint / 2, hue),
    "--term-cursor": ok(0.82, 0.1, hue),
    "--term-selection": ok(0.82, 0.1, hue, 0.25),
    "--shadow-1": "0 1px 2px rgba(0, 0, 0, 0.4)",
    "--shadow-2": "0 12px 34px rgba(0, 0, 0, 0.55)",
    "--shadow-pop": "0 8px 24px rgba(0, 0, 0, 0.3)",
  };
  const light: Record<string, string> = {
    "--bg-base": ok(0.975, tint, hue),
    "--surface-1": ok(0.975, tint, hue),
    "--surface-2": ok(0.995, tint / 2, hue),
    "--surface-3": ok(0.94, tint, hue),
    "--bg-terminal": ok(0.975, tint, hue),
    "--accent": ok(sp.light.l, sp.light.c, hue),
    "--accent-hover": ok(sp.light.l - 0.06, sp.light.c, hue),
    "--accent-dim": "rgba(0, 0, 0, 0.05)",
    "--on-accent": onAccent(sp.light.l),
    "--text-1": ok(0.22, tint, hue),
    "--text-2": ok(0.46, tint, hue),
    "--text-3": ok(0.6, tint, hue),
    "--border-0": "rgba(0, 0, 0, 0.05)",
    "--border-1": "rgba(0, 0, 0, 0.08)",
    "--border-2": "rgba(0, 0, 0, 0.13)",
    "--ink": "0, 0, 0",
    "--term-fg": ok(0.25, tint, hue),
    "--term-cursor": ok(sp.light.l, sp.light.c, hue),
    "--term-selection": ok(sp.light.l, sp.light.c, hue, 0.22),
    "--shadow-1": "0 1px 2px rgba(0, 0, 0, 0.08)",
    "--shadow-2": "0 12px 34px rgba(0, 0, 0, 0.16)",
    "--shadow-pop": "0 8px 24px rgba(0, 0, 0, 0.12)",
  };
  return { id, name, blurb, vars: { dark, light } };
}

export const THEMES: Theme[] = [
  // id stays "t3": it is the persisted default in everyone's saved settings
  build("t3", "HyprSpace", "Neutral", { hue: 264, tint: 0, dark: { l: 0.488, c: 0.217 }, light: { l: 0.45, c: 0.2 } }),
  build("orchid", "Orchid", "Pink", { hue: 350, tint: 0.012, dark: { l: 0.62, c: 0.22 }, light: { l: 0.5, c: 0.22 } }),
  build("grove", "Grove", "Green", { hue: 155, tint: 0.012, dark: { l: 0.68, c: 0.15 }, light: { l: 0.5, c: 0.14 } }),
  build("ocean", "Ocean", "Blue", { hue: 235, tint: 0.012, dark: { l: 0.68, c: 0.15 }, light: { l: 0.5, c: 0.17 } }),
  build("ember", "Ember", "Orange", { hue: 45, tint: 0.012, dark: { l: 0.72, c: 0.16 }, light: { l: 0.55, c: 0.17 } }),
  build("iris", "Iris", "Violet", { hue: 300, tint: 0.012, dark: { l: 0.65, c: 0.2 }, light: { l: 0.5, c: 0.2 } }),
];

export function themeById(id: string): Theme {
  return THEMES.find((t) => t.id === id) ?? THEMES[0];
}

export function resolveMode(scheme: Scheme): Mode {
  if (scheme !== "system") return scheme;
  return window.matchMedia?.("(prefers-color-scheme: light)").matches ? "light" : "dark";
}

// Paint a theme onto the root and say which side it landed on.
export function applyTheme(id: string, scheme: Scheme = "dark"): Mode {
  const mode = resolveMode(scheme);
  const root = document.documentElement;
  // suppress transitions during the swap so colors snap instead of every element animating (T3 trick)
  root.classList.add("no-transitions");
  for (const [k, val] of Object.entries(themeById(id).vars[mode])) {
    root.style.setProperty(k, val);
  }
  root.dataset.scheme = mode;
  root.style.colorScheme = mode;
  requestAnimationFrame(() => requestAnimationFrame(() => root.classList.remove("no-transitions")));
  return mode;
}
