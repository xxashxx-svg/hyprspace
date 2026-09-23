import { useEffect, useRef, useState, type PointerEvent as RPointerEvent } from "react";
import { ChevronDown, ChevronUp, GripVertical, Paperclip, X } from "lucide-react";
import { readImageFile } from "../../api";

// Where the tray sits inside its pane once dragged, and which way its hover preview should open
// so it never runs off the pane's edge.
interface Spot {
  x: number;
  y: number;
  up: boolean;
  left: boolean;
}

// The images pasted into a pane since its last Enter. The terminal only ever shows a path or
// claude's [Image #N], so this is the one place you can see what the agent is about to get.
// It stays mounted while empty, so a drag or a collapse sticks for the pane's life.
export function PasteTray({
  paths,
  flush,
  onOpen,
  onDismiss,
  onClear,
}: {
  paths: string[];
  flush: boolean; // no pane header above (a tab in a group), so sit at the top edge
  onOpen: (path: string) => void;
  onDismiss: (path: string) => void;
  onClear: () => void;
}) {
  const [open, setOpen] = useState(true);
  const [spot, setSpot] = useState<Spot | null>(null);
  const ref = useRef<HTMLDivElement>(null);

  // drag by the header, clamped inside the pane
  const grab = (e: RPointerEvent) => {
    const el = ref.current;
    const box = el?.offsetParent as HTMLElement | null;
    if (!el || !box || e.button !== 0) return;
    e.preventDefault();
    const r = el.getBoundingClientRect();
    const b = box.getBoundingClientRect();
    const dx = e.clientX - r.left;
    const dy = e.clientY - r.top;
    const move = (ev: PointerEvent) => {
      const x = Math.min(Math.max(ev.clientX - b.left - dx, 6), b.width - r.width - 6);
      const y = Math.min(Math.max(ev.clientY - b.top - dy, 6), b.height - r.height - 6);
      setSpot({ x, y, up: y + r.height / 2 > b.height / 2, left: x + r.width / 2 < b.width / 2 });
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };

  if (!paths.length) return null;
  const cls = ["ptray", flush && !spot && "flush", !open && "folded", spot?.up && "up", spot?.left && "left"]
    .filter(Boolean)
    .join(" ");
  return (
    <div ref={ref} className={cls} style={spot ? { left: spot.x, top: spot.y, right: "auto" } : undefined}>
      <div className="ptray-head" onPointerDown={grab} onDoubleClick={() => setOpen((o) => !o)} title="Drag to move">
        <GripVertical size={12} className="ptray-grip" />
        <Paperclip size={12} className="ptray-clip" />
        <span className="ptray-title">Attached</span>
        <span className="ptray-count">{paths.length}</span>
        <span className="ptray-gap" />
        <button className="ptray-btn" title={open ? "Collapse" : "Expand"} onPointerDown={(e) => e.stopPropagation()} onClick={() => setOpen((o) => !o)}>
          {open ? <ChevronUp size={13} /> : <ChevronDown size={13} />}
        </button>
        <button className="ptray-btn" title="Clear the tray" onPointerDown={(e) => e.stopPropagation()} onClick={onClear}>
          <X size={12} />
        </button>
      </div>
      {open && (
        <>
          <div className="ptray-row">
            {paths.map((p) => (
              <Thumb key={p} path={p} onOpen={onOpen} onDismiss={onDismiss} />
            ))}
          </div>
          <div className="ptray-hint">Goes in with your next message</div>
        </>
      )}
    </div>
  );
}

function Thumb({ path, onOpen, onDismiss }: { path: string; onOpen: (p: string) => void; onDismiss: (p: string) => void }) {
  const [src, setSrc] = useState<string | null>(null);
  const [size, setSize] = useState("");
  useEffect(() => {
    let live = true;
    readImageFile(path)
      .then((s) => live && setSrc(s))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [path]);

  return (
    <div className="ptray-item">
      <button className="ptray-shot" title="Open full size" onClick={() => onOpen(path)}>
        {src && <img src={src} alt="" onLoad={(e) => setSize(`${e.currentTarget.naturalWidth} × ${e.currentTarget.naturalHeight}`)} />}
      </button>
      <span className="ptray-size">{size || " "}</span>
      <button className="ptray-x" title="Dismiss" onClick={() => onDismiss(path)}>
        <X size={10} strokeWidth={2.5} />
      </button>
      {src && (
        <div className="ptray-peek">
          <img src={src} alt="" />
          <div className="ptray-peek-cap">
            <span>{size}</span>
            <span>Click to open</span>
          </div>
        </div>
      )}
    </div>
  );
}
