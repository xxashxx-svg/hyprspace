import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { FolderOpen, Maximize2, Minus, Plus, RotateCw, Scan, X } from "lucide-react";
import { readImageFile, revealPath } from "../api";

interface Props {
  path: string;
  active: boolean;
  onClose: () => void;
  /** the tab strip already shows the filename and a close × — don't repeat them */
  tabbed?: boolean;
}

const MIN = 0.05;
const MAX = 32;
const clamp = (z: number) => Math.max(MIN, Math.min(MAX, z));
// a drag this small is a click, not a pan — otherwise click-to-zoom never fires
const DRAG_SLOP = 4;

export function ImageViewer({ path, active, onClose, tabbed }: Props) {
  const [src, setSrc] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  // null = fit to the pane (the default). A number is an explicit zoom, 1 being 100%.
  const [zoom, setZoom] = useState<number | null>(null);
  const [nat, setNat] = useState({ w: 0, h: 0 });
  const [stage, setStage] = useState({ w: 0, h: 0 });
  const [panning, setPanning] = useState(false);
  const [nonce, setNonce] = useState(0); // bumped by Retry to re-read the file

  const stageRef = useRef<HTMLDivElement>(null);
  const imgRef = useRef<HTMLImageElement>(null);
  // Where the cursor was, and which point of the IMAGE it was over (as a 0..1 fraction), so the zoom
  // can put that same point back under the cursor. Anchoring to the image rather than to scroll
  // position is what keeps this right while the image is flex-centered: the centering margin shrinks
  // as the image grows, and scroll coords alone would drift by exactly that much.
  const anchor = useRef<{ cx: number; cy: number; fx: number; fy: number } | null>(null);

  // only hold the bytes while this tab is on screen. hidden tabs stay mounted, and a data URL is
  // ~1.33x the file, so a few open images would otherwise sit on tens of MB each for nothing.
  useEffect(() => {
    if (!active) {
      setSrc(null);
      setErr(null);
      return;
    }
    let alive = true;
    setSrc(null);
    setErr(null);
    setZoom(null);
    readImageFile(path)
      .then((url) => alive && setSrc(url))
      .catch((e) => alive && setErr(String(e)));
    return () => {
      alive = false;
    };
  }, [path, active, nonce]);

  // Stage size in state rather than read off the ref at render: the fit percentage has to recompute
  // when the pane resizes, and a ref read wouldn't re-render to update the label.
  useEffect(() => {
    const st = stageRef.current;
    if (!st) return;
    const ro = new ResizeObserver(([e]) => setStage({ w: e.contentRect.width, h: e.contentRect.height }));
    ro.observe(st);
    return () => ro.disconnect();
  }, [err]);

  // What the image is actually drawn at right now. In fit mode CSS decides (contain, never upscaling
  // past 1:1), so zooming out of fit has to start from that same number or the first notch jumps.
  const fitScale = useCallback(() => {
    if (!nat.w || !nat.h || !stage.w) return 1;
    return Math.min(1, stage.w / nat.w, stage.h / nat.h);
  }, [nat, stage]);
  const scale = zoom ?? fitScale();

  // zoom keeping `client` (a viewport point, i.e. the cursor) over the same bit of the image
  const zoomTo = useCallback(
    (next: number, client?: { x: number; y: number }) => {
      const to = clamp(next);
      // already there (holding the wheel at max, say) — bail without arming an anchor, since the
      // layout effect only runs when `zoom` changes and a stale one would fire on the NEXT zoom
      if (to === zoom) return;
      const img = imgRef.current;
      if (img && client) {
        const r = img.getBoundingClientRect();
        if (r.width && r.height) {
          anchor.current = {
            cx: client.x,
            cy: client.y,
            fx: (client.x - r.left) / r.width,
            fy: (client.y - r.top) / r.height,
          };
        }
      }
      setZoom(to);
    },
    [zoom],
  );

  // The new size has landed in the DOM by now: measure where the anchored point actually ended up
  // and scroll by the difference. Measuring beats predicting — it stays correct through centering,
  // scrollbars appearing, and the browser clamping scroll at the edges.
  useLayoutEffect(() => {
    const st = stageRef.current;
    const img = imgRef.current;
    const a = anchor.current;
    anchor.current = null;
    if (!st || !img || !a) return;
    const r = img.getBoundingClientRect();
    st.scrollLeft += r.left + a.fx * r.width - a.cx;
    st.scrollTop += r.top + a.fy * r.height - a.cy;
  }, [zoom]);

  // Wheel to zoom. Bound by hand because it has to be non-passive to preventDefault — otherwise the
  // gesture scrolls the pane underneath, and on a trackpad it fights the pinch.
  useEffect(() => {
    const st = stageRef.current;
    if (!st || !src) return;
    const onWheel = (e: WheelEvent) => {
      // a plain wheel scrolls an image that's already zoomed in; ctrl/⌘ always means zoom, and is
      // what a trackpad pinch sends
      if (!(e.ctrlKey || e.metaKey || zoom === null)) return;
      e.preventDefault();
      const step = Math.exp(-e.deltaY * 0.002); // exponential, so every notch feels the same size
      zoomTo((zoom ?? fitScale()) * step, { x: e.clientX, y: e.clientY });
    };
    st.addEventListener("wheel", onWheel, { passive: false });
    return () => st.removeEventListener("wheel", onWheel);
  }, [src, zoom, zoomTo, fitScale]);

  // drag to pan, with pointer capture so a fast drag that leaves the stage still tracks
  const drag = useRef<{ x: number; y: number; sl: number; st: number; moved: boolean } | null>(null);
  const onPointerDown = (e: React.PointerEvent) => {
    const st = stageRef.current;
    if (!st || e.button !== 0) return;
    drag.current = { x: e.clientX, y: e.clientY, sl: st.scrollLeft, st: st.scrollTop, moved: false };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  };
  const onPointerMove = (e: React.PointerEvent) => {
    const d = drag.current;
    const st = stageRef.current;
    if (!d || !st) return;
    const dx = e.clientX - d.x;
    const dy = e.clientY - d.y;
    if (!d.moved && Math.hypot(dx, dy) < DRAG_SLOP) return;
    if (!d.moved) {
      d.moved = true;
      setPanning(true);
    }
    st.scrollLeft = d.sl - dx;
    st.scrollTop = d.st - dy;
  };
  const onPointerUp = (e: React.PointerEvent) => {
    const d = drag.current;
    drag.current = null;
    setPanning(false);
    (e.currentTarget as HTMLElement).releasePointerCapture?.(e.pointerId);
    // a click (rather than a drag) still toggles fit ↔ 100%, the way it always has
    if (d && !d.moved) {
      if (zoom === null) zoomTo(1, { x: e.clientX, y: e.clientY });
      else setZoom(null);
    }
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "+" || e.key === "=") zoomTo(scale * 1.25);
    else if (e.key === "-" || e.key === "_") zoomTo(scale / 1.25);
    else if (e.key === "0") setZoom(null);
    else if (e.key === "1") setZoom(1);
    else return;
    e.preventDefault();
  };

  const name = path.split(/[\\/]/).filter(Boolean).pop() ?? path;
  // reveal-in-folder wants the containing directory (reveal_path opens a folder, not a file)
  const sep = path.includes("\\") ? "\\" : "/";
  const dir = path.split(/[\\/]/).slice(0, -1).join(sep) || path;
  const zoomed = zoom !== null;

  return (
    <div className="image-viewer">
      <div className="iv-bar">
        {!tabbed && (
          <span className="iv-name" title={path}>
            {name}
          </span>
        )}
        <span className="iv-gap" />
        {src && (
          <div className="iv-zoom">
            <button className="iv-btn" title="Zoom out (−)" onClick={() => zoomTo(scale / 1.25)}>
              <Minus size={13} />
            </button>
            <button className="iv-btn iv-pct" title="Actual size (1)" onClick={() => setZoom(1)}>
              {Math.round(scale * 100)}%
            </button>
            <button className="iv-btn" title="Zoom in (+)" onClick={() => zoomTo(scale * 1.25)}>
              <Plus size={13} />
            </button>
            <button
              className={`iv-btn${zoomed ? "" : " on"}`}
              title="Fit to pane (0)"
              onClick={() => setZoom(null)}
            >
              <Scan size={13} />
            </button>
            <button
              className={`iv-btn${zoom === 1 ? " on" : ""}`}
              title="Actual size (1)"
              onClick={() => setZoom(1)}
            >
              <Maximize2 size={13} />
            </button>
          </div>
        )}
        <button className="iv-btn" title="Reveal in folder" onClick={() => void revealPath(dir).catch(() => {})}>
          <FolderOpen size={13} /> Reveal
        </button>
        {!tabbed && (
          <button className="iv-btn iv-close" title="Close image" onClick={onClose}>
            <X size={13} /> Close
          </button>
        )}
      </div>
      {err ? (
        <div className="iv-error">
          <div className="iv-error-msg">couldn't open image</div>
          <div className="iv-error-path">{path}</div>
          <div className="iv-error-detail">{err}</div>
          <button className="iv-btn iv-retry" onClick={() => setNonce((n) => n + 1)}>
            <RotateCw size={13} /> Retry
          </button>
        </div>
      ) : (
        <div
          ref={stageRef}
          className={`iv-stage${zoomed ? " zoomed" : ""}${panning ? " panning" : ""}`}
          tabIndex={0}
          onKeyDown={onKeyDown}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onPointerCancel={onPointerUp}
        >
          {src ? (
            <img
              ref={imgRef}
              className="iv-img"
              src={src}
              alt={name}
              draggable={false}
              style={zoomed && nat.w ? { width: nat.w * zoom, height: nat.h * zoom } : undefined}
              onLoad={(e) => setNat({ w: e.currentTarget.naturalWidth, h: e.currentTarget.naturalHeight })}
              onError={() => setErr("failed to decode image")}
            />
          ) : (
            active && <div className="iv-loading">loading…</div>
          )}
        </div>
      )}
    </div>
  );
}
