import { useEffect } from "react";
import { useUi } from "../stores/ui";
import {
  newTerminal,
  closeFocused,
  toggleMaxFocused,
  switchSpaceByIndex,
  cycleSpace,
  cyclePane,
} from "../actions";

// Global hotkeys. Listens in the CAPTURE phase so it beats xterm's own key handling,
// and only swallows the combos it actually owns — everything else falls through to the
// focused terminal. We stick to Ctrl+Shift+* and Ctrl+digits so we never shadow the
// single-Ctrl control codes shells rely on (Ctrl+C/W/K/L, etc.).
export function Hotkeys() {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
      if (!mod) return;
      const take = () => {
        e.preventDefault();
        e.stopPropagation();
      };

      // Ctrl+K opens the command palette — but NOT while a terminal is focused, where
      // Ctrl+K is a real terminal key (kill-to-end-of-line); there it passes through.
      if (!e.shiftKey && !e.altKey && e.code === "KeyK") {
        if ((e.target as HTMLElement | null)?.closest?.(".xterm")) return;
        take();
        useUi.getState().togglePalette();
        return;
      }

      if (e.shiftKey) {
        switch (e.code) {
          case "KeyP":
            take();
            useUi.getState().togglePalette();
            return;
          case "KeyT":
            take();
            void newTerminal();
            return;
          case "KeyW":
            take();
            closeFocused();
            return;
          case "KeyM":
            take();
            toggleMaxFocused();
            return;
          case "KeyG":
            take();
            useUi.getState().toggleDock();
            return;
          case "ArrowRight":
            take();
            cyclePane(1);
            return;
          case "ArrowLeft":
            take();
            cyclePane(-1);
            return;
        }
      }

      if (e.code === "Tab") {
        take();
        cycleSpace(e.shiftKey ? -1 : 1);
        return;
      }

      if (!e.shiftKey && e.code.startsWith("Digit")) {
        const n = Number(e.code.slice(5));
        if (n >= 1 && n <= 9) {
          take();
          switchSpaceByIndex(n - 1);
        }
      }
    };
    document.addEventListener("keydown", onKey, true);
    return () => document.removeEventListener("keydown", onKey, true);
  }, []);
  return null;
}
