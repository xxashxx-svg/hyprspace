import { useRef, useState, type ReactNode } from "react";
import { ChevronRight } from "lucide-react";

// rough panel size, only used to decide which side/direction to open toward
const PANEL_W = 200;
const PANEL_H = 250;

// A context-menu row that opens a nested panel on hover. Used to fold the long provider list
// ("Open Claude here", "Open Gemini here", …) into a single "Open here ▸" row.
// Closing is delayed a beat so the diagonal mouse travel from the row to the panel doesn't dismiss it.
export function CtxSubmenu({
  label,
  icon,
  children,
}: {
  label: string;
  icon?: ReactNode;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const [up, setUp] = useState(false); // flip above the row when there's no room below
  const [left, setLeft] = useState(false); // ...and to the other side when there's none to the right
  const rowRef = useRef<HTMLDivElement>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const show = () => {
    clearTimeout(timer.current);
    const r = rowRef.current?.getBoundingClientRect();
    if (r) {
      setUp(r.bottom + PANEL_H > window.innerHeight);
      // the files dock lives at the right edge, so the default right-hand panel lands off-screen
      setLeft(r.right + PANEL_W > window.innerWidth);
    }
    setOpen(true);
  };
  const hide = () => {
    clearTimeout(timer.current);
    timer.current = setTimeout(() => setOpen(false), 160);
  };

  return (
    <div className="ctx-sub" ref={rowRef} onMouseEnter={show} onMouseLeave={hide}>
      <button className="ctx-item" onClick={show}>
        {icon}
        <span>{label}</span>
        <ChevronRight size={13} className="ctx-sub-caret" />
      </button>
      {open && (
        <div
          className={`ctx-menu ctx-sub-pop${up ? " up" : ""}${left ? " left" : ""}`}
          onMouseEnter={show}
          onMouseLeave={hide}
        >
          {children}
        </div>
      )}
    </div>
  );
}
