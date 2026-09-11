import { useCallback, useEffect, useRef, useState } from "react";
import { FolderOpen, Maximize2, Minus, Plus, RotateCw, Scan, X } from "lucide-react";
import { readImageFile, revealPath } from "../api";
import { LoadingState } from "./LoadingState";

interface Props {
  path: string;
  active: boolean;
  onClose: () => void;
  /** the pane header already shows the file name and a close button */
  tabbed?: boolean;
}

const MIN = 0.02;
const MAX = 64;
const STEP = 1.25;
const DRAG_SLOP = 4;

/** Where the image sits: its scale, and its center's offset from the stage center, in pixels. */
interface View {
  s: number;
  x: number;
  y: number;
}

const clampScale = (s: number) => Math.max(MIN, Math.min(MAX, s));

/**
 * Image viewer. The image is drawn at natural size and moved with a transform, so zooming is one
 * scale value and panning is one offset. The wheel always zooms toward the cursor, dragging always
 * pans, double-click toggles between fit and 100%, and the offset is clamped so the image can never
 * leave the pane.
 */
export function ImageViewer({ path, active, onClose, tabbed }: Props) {
  const [src, setSrc] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [nat, setNat] = useState({ w: 0, h: 0 });
  const [stage, setStage] = useState({ w: 0, h: 0 });
  const [view, setView] = useState<View>({ s: 1, x: 0, y: 0 });
  const [fit, setFit] = useState(true); // follow the pane size until the user zooms
  const [animate, setAnimate] = useState(false); // ease button and key zooms, not wheel or drag
  const [panning, setPanning] = useState(false);
  const [nonce, setNonce] = useState(0);
  const stageRef = useRef<HTMLDivElement>(null);

  // hold the bytes only while on screen: hidden panes stay mounted, and a data url is 1.3x the file
  useEffect(() => {
    if (!active) {
      setSrc(null);
      setErr(null);
      return;
    }
    let alive = true;
    setSrc(null);
    setErr(null);
    setFit(true);
    readImageFile(path)
      .then((url) => alive && setSrc(url))
      .catch((e) => alive && setErr(String(e)));
    return () => {
      alive = false;
    };
  }, [path, active, nonce]);

  useEffect(() => {
    const st = stageRef.current;
    if (!st) return;
    const ro = new ResizeObserver(([e]) => setStage({ w: e.contentRect.width, h: e.contentRect.height }));
    ro.observe(st);
    return () => ro.disconnect();
  }, [err]);

  const fitScale = useCallback(() => {
    if (!nat.w || !nat.h || !stage.w || !stage.h) return 1;
    return Math.min(1, stage.w / nat.w, stage.h / nat.h);
  }, [nat, stage]);

  // the image can go anywhere, as long as a sliver of it stays inside the pane
  const clampView = useCallback(
    (v: View): View => {
      const w = nat.w * v.s;
      const h = nat.h * v.s;
      const keep = Math.max(16, Math.min(w, h, 64));
      const mx = Math.max(0, (stage.w + w) / 2 - keep);
      const my = Math.max(0, (stage.h + h) / 2 - keep);
      return { s: v.s, x: Math.max(-mx, Math.min(mx, v.x)), y: Math.max(-my, Math.min(my, v.y)) };
    },
    [nat, stage],
  );

  // in fit mode the view follows the pane and the image size
  useEffect(() => {
    if (fit) setView({ s: fitScale(), x: 0, y: 0 });
  }, [fit, fitScale]);

  /** Zoom to `s`, keeping the point under `client` (a viewport coordinate) still. */
  const zoomTo = useCallback(
    (s: number, client?: { x: number; y: number }, eased = false) => {
      const st = stageRef.current;
      const next = clampScale(s);
      setFit(false);
      setAnimate(eased);
      setView((v) => {
        if (!st) return { ...v, s: next };
        const r = st.getBoundingClientRect();
        // cursor relative to the stage center, falling back to the center itself
        const px = client ? client.x - (r.left + r.width / 2) : 0;
        const py = client ? client.y - (r.top + r.height / 2) : 0;
        const k = next / v.s;
        return clampView({ s: next, x: px - (px - v.x) * k, y: py - (py - v.y) * k });
      });
    },
    [clampView],
  );
  const toFit = () => {
    setAnimate(true);
    setFit(true);
  };
  const toActual = (client?: { x: number; y: number }) => zoomTo(1, client, true);

  // the wheel zooms, always. Bound by hand so it can be non-passive and stop the pane scrolling.
  useEffect(() => {
    const st = stageRef.current;
    if (!st || !src) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      // trackpad pinch arrives as ctrl+wheel with small deltas; a mouse notch is about 100
      const factor = Math.exp(-e.deltaY * (e.ctrlKey ? 0.01 : 0.0022));
      zoomTo(view.s * factor, { x: e.clientX, y: e.clientY });
    };
    st.addEventListener("wheel", onWheel, { passive: false });
    return () => st.removeEventListener("wheel", onWheel);
  }, [src, view.s, zoomTo]);

  // drag to pan
  const drag = useRef<{ x: number; y: number; vx: number; vy: number; moved: boolean } | null>(null);
  const onPointerDown = (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    drag.current = { x: e.clientX, y: e.clientY, vx: view.x, vy: view.y, moved: false };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  };
  const onPointerMove = (e: React.PointerEvent) => {
    const d = drag.current;
    if (!d) return;
    const dx = e.clientX - d.x;
    const dy = e.clientY - d.y;
    if (!d.moved && Math.hypot(dx, dy) < DRAG_SLOP) return;
    if (!d.moved) {
      d.moved = true;
      setPanning(true);
      setAnimate(false);
    }
    setView((v) => clampView({ s: v.s, x: d.vx + dx, y: d.vy + dy }));
  };
  const onPointerUp = (e: React.PointerEvent) => {
    drag.current = null;
    setPanning(false);
    (e.currentTarget as HTMLElement).releasePointerCapture?.(e.pointerId);
  };
  const onDoubleClick = (e: React.MouseEvent) => {
    if (fit || Math.abs(view.s - fitScale()) < 0.001) toActual({ x: e.clientX, y: e.clientY });
    else toFit();
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "+" || e.key === "=") zoomTo(view.s * STEP, undefined, true);
    else if (e.key === "-" || e.key === "_") zoomTo(view.s / STEP, undefined, true);
    else if (e.key === "0") toFit();
    else if (e.key === "1") toActual();
    else return;
    e.preventDefault();
  };

  const name = path.split(/[\\/]/).filter(Boolean).pop() ?? path;
  const sep = path.includes("\\") ? "\\" : "/";
  const dir = path.split(/[\\/]/).slice(0, -1).join(sep) || path;
  const atActual = Math.abs(view.s - 1) < 0.001;
  const canPan = !!src && nat.w > 0;

  return (
    <div className="image-viewer">
      <div className="iv-bar">
        {!tabbed && (
          <span className="iv-name" title={path}>
            {name}
          </span>
        )}
        {src && nat.w > 0 && (
          <span className="iv-dims">
            {nat.w} × {nat.h}
          </span>
        )}
        <span className="iv-gap" />
        {src && (
          <div className="iv-zoom">
            <button className="iv-btn" title="Zoom out (-)" onClick={() => zoomTo(view.s / STEP, undefined, true)}>
              <Minus size={13} />
            </button>
            <button className="iv-btn iv-pct" title="Actual size (1)" onClick={() => toActual()}>
              {Math.round(view.s * 100)}%
            </button>
            <button className="iv-btn" title="Zoom in (+)" onClick={() => zoomTo(view.s * STEP, undefined, true)}>
              <Plus size={13} />
            </button>
            <button className={`iv-btn${fit ? " on" : ""}`} title="Fit to pane (0)" onClick={toFit}>
              <Scan size={13} />
            </button>
            <button className={`iv-btn${atActual && !fit ? " on" : ""}`} title="Actual size (1)" onClick={() => toActual()}>
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
          <div className="iv-error-msg">Could not open the image</div>
          <div className="iv-error-path">{path}</div>
          <div className="iv-error-detail">{err}</div>
          <button className="iv-btn iv-retry" onClick={() => setNonce((n) => n + 1)}>
            <RotateCw size={13} /> Retry
          </button>
        </div>
      ) : (
        <div
          ref={stageRef}
          className={`iv-stage${canPan ? " pannable" : ""}${panning ? " panning" : ""}`}
          tabIndex={0}
          onKeyDown={onKeyDown}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onPointerCancel={onPointerUp}
          onDoubleClick={onDoubleClick}
        >
          {src ? (
            <img
              className={`iv-img${animate ? " eased" : ""}`}
              src={src}
              alt={name}
              draggable={false}
              style={{
                width: nat.w || undefined,
                height: nat.h || undefined,
                transform: `translate(-50%, -50%) translate(${view.x}px, ${view.y}px) scale(${view.s})`,
                opacity: nat.w ? 1 : 0,
              }}
              onLoad={(e) => setNat({ w: e.currentTarget.naturalWidth, h: e.currentTarget.naturalHeight })}
              onError={() => setErr("The file is not an image this viewer can decode.")}
            />
          ) : (
            active && <LoadingState label="Opening image" variant="dots" />
          )}
        </div>
      )}
    </div>
  );
}
