import { useEffect } from "react";
import { useLoops } from "../stores/loops";
import { startLoop, isLoopActive } from "../lib/automations";

// Mounts once. Hydrates saved automations, then arms every enabled SCHEDULED one — that's what the
// "Arm automatically when HyprSpace opens" toggle promises. Manual automations never auto-run.
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
