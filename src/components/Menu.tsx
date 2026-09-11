import { useEffect, useLayoutEffect, useRef, type MouseEvent as RMouseEvent, type ReactNode, type RefObject } from "react";
import { createPortal } from "react-dom";
import { Search } from "lucide-react";

// a dropdown pinned to an anchor element. portaled to <body> and fixed-positioned from the
// anchor's rect, so neither a scrolling container (the sidebar) nor a backdrop-filter box (the
// composer) can clip or re-anchor it. arrow keys move a highlight over the rows, Enter picks,
// Escape / outside click closes — clicks on the anchor itself are left to the anchor, whose own
// handler toggles. rows are plain `.pop-menu-item` buttons, so the menu doesn't need to know them.
export function Menu({
  open,
  anchorRef,
  onClose,
  up,
  search,
  hint,
  compact,
  children,
}: {
  open: boolean;
  anchorRef: RefObject<HTMLElement | null>;
  onClose: () => void;
  up?: boolean; // open upward (for a chip near the bottom of its box)
  search?: { value: string; onChange: (v: string) => void; placeholder?: string };
  hint?: boolean; // the "↑↓ ↵ esc" footer
  compact?: boolean; // one-line rows, narrower
  children: ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // place it before paint, straight on the element — no state round-trip. kept on screen: never
  // past the right edge, and never taller than the room below (or above) the anchor
  useLayoutEffect(() => {
    const el = ref.current;
    const a = anchorRef.current;
    if (!open || !el || !a) return;
    const r = a.getBoundingClientRect();
    const left = Math.max(8, Math.min(r.left, window.innerWidth - el.offsetWidth - 8));
    el.style.left = `${left}px`;
    if (up) {
      el.style.bottom = `${window.innerHeight - r.top + 6}px`;
      el.style.maxHeight = `${r.top - 14}px`;
    } else {
      el.style.top = `${r.bottom + 6}px`;
      el.style.maxHeight = `${window.innerHeight - r.bottom - 14}px`;
    }
  }, [open, anchorRef, up]);

  useEffect(() => {
    if (open) inputRef.current?.focus();
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const items = () => Array.from(ref.current?.querySelectorAll<HTMLElement>(".pop-menu-item") ?? []);
    const mark = (list: HTMLElement[], idx: number) => {
      list.forEach((el, i) => el.classList.toggle("active", i === idx));
      list[idx]?.scrollIntoView({ block: "nearest" });
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
        return;
      }
      const list = items();
      if (!list.length) return;
      const cur = list.findIndex((el) => el.classList.contains("active"));
      if (e.key === "ArrowDown") {
        e.preventDefault();
        mark(list, (cur + 1) % list.length);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        mark(list, (cur - 1 + list.length) % list.length);
      } else if (e.key === "Enter") {
        // with a filter typed, Enter takes the first match even before you arrow to it
        const hit = cur >= 0 ? list[cur] : search?.value ? list[0] : undefined;
        if (hit) {
          e.preventDefault();
          hit.click();
        }
      }
    };
    const onDown = (e: MouseEvent) => {
      const t = e.target as Node;
      if (ref.current?.contains(t) || anchorRef.current?.contains(t)) return;
      onClose();
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open, anchorRef, onClose, search?.value]);

  // the mouse and the keys agree on one highlight
  const onOver = (e: RMouseEvent) => {
    const t = (e.target as HTMLElement).closest<HTMLElement>(".pop-menu-item");
    if (!t) return;
    for (const el of ref.current?.querySelectorAll(".pop-menu-item.active") ?? []) el.classList.remove("active");
    t.classList.add("active");
  };

  if (!open) return null;
  return createPortal(
    <div className={`pop-menu${compact ? " compact" : ""}`} ref={ref} onMouseOver={onOver}>
      {search && (
        <div className="pop-menu-search">
          <Search size={13} />
          <input
            ref={inputRef}
            value={search.value}
            placeholder={search.placeholder ?? "Search…"}
            spellCheck={false}
            onChange={(e) => search.onChange(e.target.value)}
          />
        </div>
      )}
      <div className="pop-menu-list">{children}</div>
      {hint && (
        <div className="pop-menu-foot">
          <span>
            <kbd>↑↓</kbd> move
          </span>
          <span>
            <kbd>↵</kbd> select
          </span>
          <span>
            <kbd>esc</kbd> close
          </span>
        </div>
      )}
    </div>,
    document.body,
  );
}
