import { useEffect, useState } from "react";
import { useConfirm } from "../stores/confirm";
import { useSettings } from "../stores/settings";

// In-app confirmation modal — matches the app theme instead of the native OS dialog.
export function ConfirmDialog() {
  const req = useConfirm((s) => s.req);
  const answer = useConfirm((s) => s.answer);
  const [dontAsk, setDontAsk] = useState(false);

  useEffect(() => setDontAsk(false), [req]); // fresh checkbox per dialog

  // remember the choice only when the user actually confirms
  const done = (ok: boolean) => {
    if (ok && dontAsk && req?.dontAskId) useSettings.getState().dismissConfirm(req.dontAskId);
    answer(ok);
  };

  useEffect(() => {
    if (!req) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") done(false);
      if (e.key === "Enter") done(true);
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [req, answer, dontAsk]);

  if (!req) return null;

  return (
    <div className="confirm-overlay" onMouseDown={() => done(false)}>
      <div className="confirm" onMouseDown={(e) => e.stopPropagation()}>
        <div className="confirm-title">{req.title}</div>
        <div className="confirm-msg">{req.message}</div>
        {req.dontAskId && (
          <label className="confirm-dontask">
            <input type="checkbox" checked={dontAsk} onChange={(e) => setDontAsk(e.target.checked)} />
            Don't ask me again
          </label>
        )}
        <div className="confirm-actions">
          <button className="btn" onClick={() => done(false)}>
            {req.cancelLabel ?? "Cancel"}
          </button>
          <button
            className={`btn ${req.danger ? "danger" : "primary"}`}
            onClick={() => done(true)}
            autoFocus
          >
            {req.confirmLabel ?? "Confirm"}
          </button>
        </div>
      </div>
    </div>
  );
}
