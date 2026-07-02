// Neutral T3-style themes: one near-black base, white-alpha lines, a single accent that varies.
// Shared bits (status colors, radii, fonts) live in tokens.css.
export interface Theme {
  id: string;
  name: string;
  blurb?: string; // short hue descriptor shown under the name in the picker
  vars: Record<string, string>;
}

// Every theme shares this neutral foundation — only the accent trio changes between them.
const base: Record<string, string> = {
  "--bg-base": "#161616",
  "--surface-1": "#161616",
  "--surface-2": "#1e1e1e",
  "--surface-3": "#282828",
  // terminal bg = the app base so the terminal blends into the surface (T3-style) instead of being
  // a distinct darker box. cursor/selection are T3's soft blue.
  "--bg-terminal": "#161616",
  "--accent-dim": "rgba(255,255,255,0.07)",
  "--text-1": "#f5f5f5",
  "--text-2": "#a1a1a1",
  "--text-3": "#767676",
  "--border-1": "rgba(255,255,255,0.06)",
  "--border-2": "rgba(255,255,255,0.10)",
  "--term-fg": "rgb(237, 241, 247)",
  "--term-cursor": "rgb(180, 203, 255)",
  "--term-selection": "rgba(180, 203, 255, 0.25)",
};

export const THEMES: Theme[] = [
  {
    // id stays "t3" — it's the persisted default in everyone's saved settings
    id: "t3",
    name: "Midnight",
    blurb: "Deep indigo",
    vars: {
      ...base,
      "--accent": "oklch(0.488 0.217 264)", // T3's exact --primary (deep indigo, used sparingly)
      "--accent-hover": "oklch(0.55 0.21 264)",
      "--on-accent": "#ffffff",
    },
  },
  {
    id: "emerald",
    name: "Emerald",
    blurb: "Cool green",
    vars: {
      ...base,
      "--accent": "oklch(0.696 0.16 162)",
      "--accent-hover": "oklch(0.76 0.15 162)",
      "--on-accent": "#04130c",
    },
  },
  {
    id: "amber",
    name: "Amber",
    blurb: "Warm gold",
    vars: {
      ...base,
      "--accent": "oklch(0.79 0.15 73)",
      "--accent-hover": "oklch(0.84 0.13 73)",
      "--on-accent": "#1c1304",
    },
  },
  {
    id: "rose",
    name: "Rose",
    blurb: "Crimson",
    vars: {
      ...base,
      "--accent": "oklch(0.65 0.22 14)",
      "--accent-hover": "oklch(0.71 0.2 14)",
      "--on-accent": "#ffffff",
    },
  },
  {
    id: "mono",
    name: "Mono",
    blurb: "Grayscale",
    vars: {
      ...base,
      "--accent": "#e6e6e6",
      "--accent-hover": "#ffffff",
      "--on-accent": "#161616",
    },
  },
];

export function applyTheme(id: string): void {
  const theme = THEMES.find((t) => t.id === id) ?? THEMES[0];
  const root = document.documentElement;
  // suppress transitions during the swap so colors snap instead of every element animating (T3 trick)
  root.classList.add("no-transitions");
  for (const [k, val] of Object.entries(theme.vars)) {
    root.style.setProperty(k, val);
  }
  requestAnimationFrame(() => requestAnimationFrame(() => root.classList.remove("no-transitions")));
}
