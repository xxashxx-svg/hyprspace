import { useEffect, type ReactNode } from "react";
import { useEntitlement } from "../stores/entitlement";
import { Paywall } from "./Paywall";

const RECHECK_MS = 6 * 60 * 60 * 1000; // re-check entitlement periodically while the app is open

// Sits inside AuthGate (so we already have a signed-in session). Runs the entitlement check and,
// ONLY if it comes back an explicit "locked" (paid mode + not entitled), shows the paywall.
// Everything else — checking, ok, errors — passes straight through (fail-open). While we're in the
// free phase the check resolves to "ok" instantly and this is invisible.
export function EntitlementGate({ children }: { children: ReactNode }) {
  const status = useEntitlement((s) => s.status);
  const check = useEntitlement((s) => s.check);

  useEffect(() => {
    void check();
    const t = setInterval(() => void check(), RECHECK_MS);
    const onFocus = () => void check();
    window.addEventListener("focus", onFocus);
    return () => {
      clearInterval(t);
      window.removeEventListener("focus", onFocus);
    };
  }, [check]);

  if (status === "locked") return <Paywall />;
  return <>{children}</>;
}
