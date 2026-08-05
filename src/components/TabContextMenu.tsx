import { useEffect, useState } from "react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { Copy, ExternalLink, Maximize2, Pencil, X, XCircle } from "lucide-react";
import { useWorkspaces } from "../stores/workspace";
import { useUi } from "../stores/ui";
import { revealPath } from "../api";
import { parentOf } from "../lib/projects";
import { closeSession } from "../actions";
import { revealLabel } from "../platform";
import { PROVIDERS } from "../lib/providers";
import { CtxSubmenu } from "./CtxSubmenu";
import { maybeAutostart } from "../lib/startup";


// Right-click a pane tab. Carries what the pane header's "…" menu used to, now that a tab strip
// replaced that header on every pane.
export function TabContextMenu({
  ctx,
  onClose,
}: {
  ctx: { x: number; y: number; wsId: string; sessionId: string };
  onClose: () => void;
}) {
  const ws = useWorkspaces((s) => s.workspaces.find((w) => w.id === ctx.wsId));
  const sess = ws?.sessions.find((ss) => ss.id === ctx.sessionId);
  const [renaming, setRenaming] = useState(false);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose]);

  if (!ws || !sess) return null;
  // a viewer tab points at a file; a terminal points at its folder
  const target = sess.image ?? sess.file ?? "";
  const folder = target ? parentOf(target) : sess.cwd || ws.cwd || "";
  const siblings = sess.group ? ws.sessions.filter((x) => x.group === sess.group) : [sess];

  const run = (fn: () => void) => {
    fn();
    onClose();
  };

  return (
    <>
      <div
        className="ctx-backdrop"
        onClick={onClose}
        onContextMenu={(e) => {
          e.preventDefault();
          onClose();
        }}
      />
      <div
        className="ctx-menu"
        style={{
          left: Math.min(ctx.x, window.innerWidth - 220),
          top: Math.min(ctx.y, Math.max(8, window.innerHeight - 320)),
        }}
      >
        {renaming ? (
          <input
            className="ctx-rename"
            autoFocus
            defaultValue={sess.title}
            onFocus={(e) => e.currentTarget.select()}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                const v = e.currentTarget.value.trim();
                if (v) useWorkspaces.getState().renameSession(sess.id, v);
                onClose();
              }
              if (e.key === "Escape") setRenaming(false);
            }}
            onBlur={onClose}
          />
        ) : (
          <>
            {folder && (
              <CtxSubmenu label="Open here" icon={<ExternalLink size={14} />}>
                {PROVIDERS.map((pr) => (
                  <button
                    key={pr.id}
                    className="ctx-item"
                    onClick={() =>
                      run(() => {
                        const st = useWorkspaces.getState();
                        st.setActive(ctx.wsId);
                        st.addSession(ctx.wsId, pr.cmd(), folder);
                        maybeAutostart(ctx.wsId);
                      })
                    }
                  >
                    <pr.icon size={14} />
                    <span>{pr.label}</span>
                  </button>
                ))}
              </CtxSubmenu>
            )}
            {folder && (
              <button className="ctx-item" onClick={() => run(() => void revealPath(folder).catch(() => {}))}>
                <ExternalLink size={14} />
                <span>{revealLabel}</span>
              </button>
            )}
            <button
              className="ctx-item"
              onClick={() => run(() => void writeText(target || folder).catch(() => {}))}
            >
              <Copy size={14} />
              <span>Copy path</span>
            </button>
            <div className="ctx-sep" />
            <button className="ctx-item" onClick={() => setRenaming(true)}>
              <Pencil size={14} />
              <span>Rename…</span>
            </button>
            <button
              className="ctx-item"
              onClick={() =>
                run(() => {
                  // bring the tab forward first — the maximized slot shows its ACTIVE tab, so
                  // maximizing a background tab without activating it would fullscreen the wrong one
                  if (sess.group) useWorkspaces.getState().setActiveTab(ctx.wsId, sess.group, sess.id);
                  useUi.getState().toggleMaximized(sess.id);
                })
              }
            >
              <Maximize2 size={14} />
              <span>Maximize</span>
            </button>
            <div className="ctx-sep" />
            <button className="ctx-item" onClick={() => run(() => void closeSession(ctx.wsId, sess.id))}>
              <X size={14} />
              <span>Close tab</span>
            </button>
            {siblings.length > 1 && (
              <button
                className="ctx-item danger"
                onClick={() =>
                  run(() => {
                    for (const o of siblings) if (o.id !== sess.id) void closeSession(ctx.wsId, o.id);
                  })
                }
              >
                <XCircle size={14} />
                <span>Close other tabs</span>
              </button>
            )}
          </>
        )}
      </div>
    </>
  );
}
