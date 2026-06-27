// quick OS sniff for UI that differs by platform (macOS uses native traffic lights;
// Windows uses our own custom window controls).
export const isMac = navigator.userAgent.includes("Mac");
export const isWindows = navigator.userAgent.includes("Windows");

// the primary modifier label for this platform — ⌘ on mac, Ctrl elsewhere
export const modKey = isMac ? "⌘" : "Ctrl";

// Format a keyboard shortcut for display on the current platform: tight ⌘/⇧/⌥ glyphs on macOS
// (the way mac shows them), the literal Ctrl/Shift/Alt elsewhere. Pass ONLY the shortcut text
// (e.g. "Ctrl+Shift+T" or "Ctrl K"), not a whole sentence. The hotkey handlers already accept Cmd
// (metaKey) — this only fixes the labels.
export function kbd(s: string): string {
  if (!isMac) return s;
  return s
    .replace(/Ctrl|Cmd/g, "⌘")
    .replace(/Shift/g, "⇧")
    .replace(/Alt|Option/g, "⌥")
    .replace(/[+\s]+/g, ""); // mac shows tight glyph runs: ⌘⇧T
}
