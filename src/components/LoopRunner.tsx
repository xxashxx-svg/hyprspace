import { useEffect } from "react";
import { useLoops } from "../stores/loops";
import { startLoop, isLoopActive } from "../lib/loops";

// Mounts once. Hydrates saved loops, then auto-starts every enabled automation — that's what the
// "Auto-start on open" toggle promises, whatever the mode (until-done/manual just run their course).
export function LoopRunner() {
  useEffect(() => {
    let cancelled = false;
    void useLoops
      .getState()
      .load()
      .then(() => {
        if (cancelled) return;
        for (const def of Object.values(useLoops.getState().loops)) {
          if (def.enabled && !isLoopActive(def.id)) {
            startLoop(def.id);
          }
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);
  return null;
}
