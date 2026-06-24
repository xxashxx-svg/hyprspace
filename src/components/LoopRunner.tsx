import { useEffect } from "react";
import { useLoops } from "../stores/loops";
import { startLoop, isLoopActive } from "../lib/loops";

// Mounts once. Hydrates saved loops, then auto-starts enabled cron/interval loops so they run for
// as long as HyprSpace is open. until-done and manual loops are started by hand from the UI.
export function LoopRunner() {
  useEffect(() => {
    let cancelled = false;
    void useLoops
      .getState()
      .load()
      .then(() => {
        if (cancelled) return;
        for (const def of Object.values(useLoops.getState().loops)) {
          if (def.enabled && (def.mode === "cron" || def.mode === "interval") && !isLoopActive(def.id)) {
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
