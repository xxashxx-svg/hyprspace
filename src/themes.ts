// Neutral T3-style themes: one near-black base, white-alpha lines, a single accent that varies.
// Shared bits (status colors, radii, fonts) live in tokens.css.
export interface Theme {
  id: string;
  name: string;
  vars: Record<string, string>;
}

// Every theme shares this neutral foundation — only the accent trio changes between them.
const base: Record<string, string> = {
  "--bg-base": "#161616",
  "--surface-1": "#161616",
  "--surface-2": "#1e1e1e",
  "--surface-3": "#282828",
  "--bg-terminal": "#121212",
  "--accent-dim": "rgba(255,255,255,0.07)",
  "--text-1": "#f5f5f5",
  "--text-2": "#a1a1a1",
  "--text-3": "#767676",
  "--border-1": "rgba(255,255,255,0.06)",
  "--border-2": "rgba(255,255,255,0.10)",
  "--term-fg": "#ececec",
  "--term-cursor": "#e6e6e6",
  "--term-selection": "rgba(255,255,255,0.16)",
};

export const THEMES: Theme[] = [
  {
    id: "t3",
    name: "T3",
    vars: {
      ...base,
      "--accent": "oklch(0.588 0.217 264)",
      "--accent-hover": "oklch(0.66 0.2 264)",
      "--on-accent": "#ffffff",
    },
  },
  {
    id: "emerald",
    name: "Emerald",
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
  for (const [k, val] of Object.entries(theme.vars)) {
    root.style.setProperty(k, val);
  }
}
