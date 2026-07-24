import { useState } from "react";
import { useUi } from "../stores/ui";
import { useWorkspaces, toSlots } from "../stores/workspace";
import { layoutsFor, activeLayoutId } from "../lib/grid";
import { LayoutGrid } from "lucide-react";

// Titlebar control to pick how the current space's panes are arranged. Shows visual thumbnails of the
// layouts available for the current pane-count (generated straight from the preset grid data, so they
// always match reality). Only renders in a space with ≥2 panes (where there's a real choice).
export function LayoutPicker() {
  const view = useUi((s) => s.view);
  const activeId = useWorkspaces((s) => s.activeId);
  const workspaces = useWorkspaces((s) => s.workspaces);
  const setLayout = useWorkspaces((s) => s.setLayout);
  const [open, setOpen] = useState(false);

  const active = workspaces.find((w) => w.id === activeId) ?? null;
  const count = active ? toSlots(active.sessions).length : 0; // tiling is by slot; a tabbed group = 1
  const presets = layoutsFor(count);
  if (view !== "space" || !active || presets.length === 0) return null;

  const currentId = activeLayoutId(count, active.layouts?.[count]);

  return (
    <div className="tb-layout">
      <button
        className={`tb-action tb-layout-btn${open ? " open" : ""}`}
        title="Pane layout"
        onClick={() => setOpen((o) => !o)}
      >
        <LayoutGrid size={14} />
      </button>
      {open && (
        <>
          <div
            className="tb-menu-backdrop"
            onClick={() => setOpen(false)}
            onContextMenu={(e) => {
              e.preventDefault();
              setOpen(false);
            }}
          />
          <div className="tb-layout-pop">
            <div className="tb-layout-head">Layout · {count} panes</div>
            <div className="tb-layout-grid">
              {presets.map((p) => (
                <button
                  key={p.id}
                  className={`tb-layout-opt${p.id === currentId ? " active" : ""}`}
                  title={p.label}
                  onClick={() => {
                    setLayout(active.id, count, p.id);
                    setOpen(false);
                  }}
                >
                  <span
                    className="tb-layout-thumb"
                    style={{ gridTemplateColumns: p.cols, gridTemplateRows: p.rows }}
                  >
                    {p.cells.map((c, i) => (
                      <span
                        key={i}
                        className="tb-layout-cell"
                        style={{ gridColumn: c.gridColumn, gridRow: c.gridRow }}
                      />
                    ))}
                  </span>
                  <span className="tb-layout-label">{p.label}</span>
                </button>
              ))}
            </div>
          </div>
        </>
      )}
    </div>
  );
}
