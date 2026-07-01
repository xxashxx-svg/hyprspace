import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { usePreview } from "../stores/preview";
import { RotateCw, ExternalLink, X } from "lucide-react";

// A docked, in-app browser preview (iframe) — opened by Actions with a preview URL, so you can watch
// your dev server come up without leaving HyprSpace. Slides in on the right; resizable.
export function PreviewPanel() {
  const open = usePreview((s) => s.open);
  const url = usePreview((s) => s.url);
  const tick = usePreview((s) => s.tick);
  const setUrl = usePreview((s) => s.setUrl);
  const reload = usePreview((s) => s.reload);
  const close = usePreview((s) => s.close);

  const [width, setWidth] = useState(560);
  const [draft, setDraft] = useState(url);
  // sync the address bar when a new action opens a different URL (tick bumps each open)
  useEffect(() => setDraft(url), [url, tick]);

  if (!open) return null;

  const go = (u: string) => {
    let v = u.trim();
    if (v && !/^https?:\/\//i.test(v) && !/^localhost|^\d/.test(v)) v = "https://" + v;
    else if (/^localhost|^\d/.test(v) && !/^https?:\/\//i.test(v)) v = "http://" + v;
    setUrl(v);
    reload();
  };

  const onDragW = (e: React.PointerEvent) => {
    e.preventDefault();
    const startX = e.clientX;
    const startW = width;
    const move = (ev: PointerEvent) => setWidth(Math.min(1100, Math.max(340, startW + (startX - ev.clientX))));
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };

  return (
    <div className="prev" style={{ width }}>
      <div className="prev-grip" onPointerDown={onDragW} title="Drag to resize" />
      <div className="prev-head">
        <button className="prev-btn" title="Reload" onClick={reload}>
          <RotateCw size={13} />
        </button>
        <input
          className="prev-url"
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") go(draft);
          }}
          placeholder="localhost:5173"
          spellCheck={false}
        />
        <button
          className="prev-btn"
          title="Open in browser"
          onClick={() => url && void openUrl(url).catch(() => {})}
        >
          <ExternalLink size={13} />
        </button>
        <button className="prev-btn" title="Close preview" onClick={close}>
          <X size={14} />
        </button>
      </div>
      {url ? (
        <iframe key={`${url}#${tick}`} className="prev-frame" src={url} title="Preview" />
      ) : (
        <div className="prev-empty">Enter a URL above, or run an action with a preview.</div>
      )}
    </div>
  );
}
