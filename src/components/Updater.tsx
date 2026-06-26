import { useEffect, useRef } from "react";
import { useUpdater } from "../stores/updater";
import { X } from "lucide-react";

const RECHECK_MS = 6 * 60 * 60 * 1000; // re-check every 6h while the app stays open
const FOCUS_THROTTLE_MS = 15 * 60 * 1000; // also re-check when you return, but not more than this often

export function Updater() {
  const phase = useUpdater((s) => s.phase);
  const detail = useUpdater((s) => s.detail);
  const pct = useUpdater((s) => s.pct);
  const update = useUpdater((s) => s.update);
  const install = useUpdater((s) => s.install);
  const dismiss = useUpdater((s) => s.dismiss);
  const lastCheck = useRef(0);

  // check on launch, on an interval, and when the window regains focus (throttled) so a shipped
  // update surfaces promptly instead of waiting out the interval
  useEffect(() => {
    const run = () => {
      lastCheck.current = Date.now();
      void useUpdater.getState().checkNow();
    };
    run();
    const id = setInterval(run, RECHECK_MS);
    const onFocus = () => {
      if (Date.now() - lastCheck.current > FOCUS_THROTTLE_MS) run();
    };
    window.addEventListener("focus", onFocus);
    return () => {
      clearInterval(id);
      window.removeEventListener("focus", onFocus);
    };
  }, []);

  // the toast only surfaces when there's actually something to act on
  if (phase !== "available" && phase !== "downloading" && phase !== "error") return null;
  const busy = phase === "downloading";

  if (phase === "error") {
    return (
      <div className="updater">
        <span className="updater-dot err" />
        <span className="updater-text">{detail || "Update failed"}</span>
        <button className="updater-btn" onClick={() => void install()}>
          Retry
        </button>
        <button className="updater-x" title="Dismiss" onClick={dismiss}>
          <X size={13} />
        </button>
      </div>
    );
  }

  if (busy) {
    return (
      <div className="updater busy">
        <div className="updater-row">
          <span className="updater-spin" />
          <span className="updater-text">{detail || "Updating…"}</span>
        </div>
        <div className="updater-progress">
          <div
            className={`updater-bar${pct < 0 ? " indet" : ""}`}
            style={pct >= 0 ? { width: `${pct}%` } : undefined}
          />
        </div>
      </div>
    );
  }

  return (
    <div className="updater">
      <span className="updater-dot" />
      <span className="updater-text">Update {update?.version} available</span>
      <button className="updater-btn" onClick={() => void install()}>
        Restart &amp; update
      </button>
      <button className="updater-x" title="Later" onClick={dismiss}>
        <X size={13} />
      </button>
    </div>
  );
}
