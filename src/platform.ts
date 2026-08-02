// quick OS sniff for UI that differs by platform (macOS uses native traffic lights; Windows and
// Linux use our own custom window controls).
export const isMac = navigator.userAgent.includes("Mac");
export const isWindows = navigator.userAgent.includes("Windows");
// Anything that isn't mac or Windows is a Linux/BSD desktop as far as this app is concerned. Derived
// rather than sniffed for "Linux": WebKitGTK's UA string varies by distro and version, and mac's own
// UA contains "X11" in some configurations.
export const isLinux = !isMac && !isWindows;

/** the file manager's name on this platform, for "Reveal in …" labels */
export const fileManager = isWindows ? "Explorer" : isMac ? "Finder" : "file manager";

// Menu wording for the two file-manager actions. Linux has no single file manager to name (could be
// Nautilus, Dolphin, Thunar…), so it gets the generic phrasing instead of "Reveal in file manager".
export const revealLabel = isLinux ? "Open containing folder" : `Reveal in ${fileManager}`;
export const openFolderLabel = isLinux ? "Open folder" : `Open folder in ${fileManager}`;

// what a runnable file looks like here — the services dropzone names examples, and .bat/.exe are
// meaningless outside Windows (see taskFromFile in lib/startup.ts for the matching run logic)
export const scriptExt = isWindows ? ".bat" : ".sh";
export const binaryLabel = isWindows ? ".exe" : "executable";

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
