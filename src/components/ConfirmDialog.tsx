import { useEffect } from "react";
import { useConfirm } from "../stores/confirm";

// In-app confirmation modal — matches the app theme instead of the native OS dialog.
export function ConfirmDialog() {
  const req = useConfirm((s) => s.req);
  const answer = useConfirm((s) => s.answer);

  useEffect(() => {
    if (!req) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") answer(false);
      if (e.key === "Enter") answer(true);
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [req, answer]);

  if (!req) return null;

  return (
    <div className="confirm-overlay" onMouseDown={() => answer(false)}>
      <div className="confirm" onMouseDown={(e) => e.stopPropagation()}>
        <div className="confirm-title">{req.title}</div>
        <div className="confirm-msg">{req.message}</div>
        <div className="confirm-actions">
          <button className="btn" onClick={() => answer(false)}>
            {req.cancelLabel ?? "Cancel"}
          </button>
          <button
            className={`btn ${req.danger ? "danger" : "primary"}`}
            onClick={() => answer(true)}
            autoFocus
          >
            {req.confirmLabel ?? "Confirm"}
          </button>
        </div>
      </div>
    </div>
  );
}
