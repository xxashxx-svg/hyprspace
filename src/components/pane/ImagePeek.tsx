import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { readImageFile } from "../../api";
import { kbd } from "../../platform";

// data urls of recently hovered images, so sweeping back over the same [Image #N] is instant
const cache = new Map<string, string>();

// An image link in the terminal ([Image #N] or a path), previewed beside the mouse. Portaled to
// body so the pane's rounded clip can't cut it off, and flipped to the other side of the cursor
// near the window edges. Mount it with key={path} so a new image starts from its own cache entry.
export function ImagePeek({ path, x, y }: { path: string; x: number; y: number }) {
  const [src, setSrc] = useState(() => cache.get(path) ?? null);
  const [size, setSize] = useState("");
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (cache.has(path)) return;
    let live = true;
    readImageFile(path)
      .then((s) => {
        if (cache.size > 40) cache.clear();
        cache.set(path, s);
        if (live) setSrc(s);
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [path]);

  const place = () => {
    const el = ref.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    const left = x + 16 + r.width > window.innerWidth - 8 ? x - 16 - r.width : x + 16;
    const top = y + 16 + r.height > window.innerHeight - 8 ? y - 16 - r.height : y + 16;
    el.style.left = `${Math.max(8, left)}px`;
    el.style.top = `${Math.max(8, top)}px`;
  };
  useLayoutEffect(place);

  if (!src) return null;
  return createPortal(
    <div className="img-peek" ref={ref}>
      <img
        src={src}
        alt=""
        onLoad={(e) => {
          setSize(`${e.currentTarget.naturalWidth} × ${e.currentTarget.naturalHeight}`);
          place();
        }}
      />
      <div className="ptray-peek-cap">
        <span>{size}</span>
        <span>{kbd("Ctrl")}+click to open</span>
      </div>
    </div>,
    document.body,
  );
}
