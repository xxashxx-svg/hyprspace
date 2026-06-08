import { useEffect } from "react";
import { useUpdater } from "../stores/updater";

const RECHECK_MS = 6 * 60 * 60 * 1000; // re-check every 6h while the app stays open

export function Updater() {
  const phase = useUpdater((s) => s.phase);
  const detail = useUpdater((s) => s.detail);
  const update = useUpdater((s) => s.update);
  const install = useUpdater((s) => s.install);
  const dismiss = useUpdater((s) => s.dismiss);

  // check once on launch, then quietly on an interval
  useEffect(() => {
    const run = () => void useUpdater.getState().checkNow();
    run();
    const id = setInterval(run, RECHECK_MS);
    return () => clearInterval(id);
  }, []);

  // the toast only surfaces when there's actually something to act on
  if (phase !== "available" && phase !== "downloading") return null;
  const busy = phase === "downloading";

  return (
    <div className="updater">
      <span className="updater-dot" />
      <span className="updater-text">{busy ? detail : `Update ${update?.version} available`}</span>
      {!busy && (
        <>
          <button className="updater-btn" onClick={() => void install()}>
            Restart &amp; update
          </button>
          <button className="updater-x" title="Later" onClick={dismiss}>
            ×
          </button>
        </>
      )}
    </div>
  );
}
