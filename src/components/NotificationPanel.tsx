import { useEffect, useRef, useState } from "react";
import { useNotifications } from "../stores/notifications";
import { useUpdater } from "../stores/updater";

function relTime(ts: number): string {
  const s = Math.max(0, Math.floor((Date.now() - ts) / 1000));
  if (s < 60) return "now";
  if (s < 3600) return `${Math.floor(s / 60)}m`;
  if (s < 86400) return `${Math.floor(s / 3600)}h`;
  return `${Math.floor(s / 86400)}d`;
}

export function NotificationPanel() {
  const items = useNotifications((s) => s.items);
  const phase = useUpdater((s) => s.phase);
  const update = useUpdater((s) => s.update);
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  const unread = items.reduce((n, i) => n + (i.read ? 0 : 1), 0);

  // surface available updates (with their release notes) as notifications
  useEffect(() => {
    if (phase === "available" && update?.version) {
      useNotifications.getState().add({
        id: "update-" + update.version,
        title: `Update ${update.version} available`,
        body: update.body?.trim() || "A new version is ready — open Settings to install.",
        kind: "update",
      });
    }
  }, [phase, update]);

  // close on outside click / Escape
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const toggle = () => {
    const next = !open;
    setOpen(next);
    if (next && unread) useNotifications.getState().markAllRead();
  };

  return (
    <div className="notif" ref={ref}>
      <button className="tb-ctl" title="Notifications" onClick={toggle}>
        <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3">
          <path d="M4 6.5a4 4 0 0 1 8 0c0 3 1 4 1.5 4.5H2.5C3 10.5 4 9.5 4 6.5z" />
          <path d="M6.5 13a1.6 1.6 0 0 0 3 0" />
        </svg>
        {unread > 0 && <span className="notif-badge" />}
      </button>
      {open && (
        <div className="notif-panel">
          <div className="notif-head">
            <span>Notifications</span>
            {items.length > 0 && (
              <button className="notif-clear" onClick={() => useNotifications.getState().clear()}>
                Clear all
              </button>
            )}
          </div>
          <div className="notif-list">
            {items.length === 0 && <div className="notif-empty">You're all caught up.</div>}
            {items.map((n) => (
              <div key={n.id} className={`notif-item k-${n.kind}`}>
                <span className="notif-rail" />
                <div className="notif-content">
                  <div className="notif-item-top">
                    <span className="notif-title">{n.title}</span>
                    <span className="notif-time">{relTime(n.ts)}</span>
                  </div>
                  {n.body && <div className="notif-body">{n.body}</div>}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
