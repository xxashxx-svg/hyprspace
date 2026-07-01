import { useEffect } from "react";
import { useUi } from "../stores/ui";
import { useWorkspaces } from "../stores/workspace";
import { useProjectConfigs, folderKey } from "../stores/projectConfig";
import { runAction } from "../lib/startup";
import {
  newTerminal,
  closeFocused,
  toggleMaxFocused,
  switchSpaceByIndex,
  cycleSpace,
  cyclePane,
} from "../actions";

// "Ctrl+Alt+Shift+K"-style label from a native keydown (matches the one the Action dialog records)
function nativeCombo(e: KeyboardEvent): string | null {
  const k = e.key;
  if (k === "Control" || k === "Alt" || k === "Shift" || k === "Meta") return null;
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Meta");
  parts.push(k.length === 1 ? k.toUpperCase() : k);
  return parts.join("+");
}

// Global hotkeys. Listens in the CAPTURE phase so it beats xterm's own key handling,
// and only swallows the combos it actually owns — everything else falls through to the
// focused terminal. We stick to Ctrl+Shift+* and Ctrl+digits so we never shadow the
// single-Ctrl control codes shells rely on (Ctrl+C/W/K/L, etc.).
export function Hotkeys() {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // project-action keybindings first — they can use Alt too, and win over the built-ins
      if (e.ctrlKey || e.metaKey || e.altKey) {
        const combo = nativeCombo(e);
        if (combo) {
          const w = useWorkspaces.getState();
          const aws = w.workspaces.find((x) => x.id === w.activeId);
          const folder = aws && aws.kind !== "open" ? aws.cwd : "";
          if (folder && aws) {
            const acts = useProjectConfigs.getState().configs[folderKey(folder)]?.startup ?? [];
            const hit = acts.find((a) => a.keybinding && a.keybinding === combo);
            if (hit) {
              e.preventDefault();
              e.stopPropagation();
              runAction(aws.id, hit);
              return;
            }
          }
        }
      }

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
