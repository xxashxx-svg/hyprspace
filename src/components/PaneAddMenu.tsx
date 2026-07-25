import { SplitSquareHorizontal } from "lucide-react";
import { useWorkspaces } from "../stores/workspace";
import { PROVIDERS } from "../lib/providers";

// the provider picker shared by the pane header + button and a slot's tab-strip +. each item opens a
// new pane as a tab in the anchor's slot (same folder); "Open as new tile" keeps the old split behavior.

interface Props {
  x: number;
  y: number;
  wsId: string;
  anchorId: string;
  anchorCommand?: string;
  cwd: string;
  onClose: () => void;
}

export function PaneAddMenu({ x, y, wsId, anchorId, anchorCommand, cwd, onClose }: Props) {
  const folder = cwd.split(/[\\/]/).filter(Boolean).pop();
  return (
    <>
      <div
        className="pane-menu-backdrop"
        onMouseDown={(e) => {
          e.stopPropagation();
          onClose();
        }}
        onContextMenu={(e) => {
          e.preventDefault();
          onClose();
        }}
      />
      <div className="pane-menu" style={{ left: x, top: y }} onMouseDown={(e) => e.stopPropagation()}>
        <div className="pane-menu-label">open in {folder ?? "this folder"}</div>
        {PROVIDERS.map((p) => (
          <button
            key={p.id}
            className="pane-menu-item"
            onClick={() => {
              useWorkspaces.getState().addTab(wsId, anchorId, p.cmd(), cwd);
              onClose();
            }}
          >
            <p.icon size={14} /> {p.label}
          </button>
        ))}
        <div className="pane-menu-sep" />
        <button
          className="pane-menu-item"
          onClick={() => {
            useWorkspaces.getState().addSession(wsId, anchorCommand, cwd);
            onClose();
          }}
        >
          <SplitSquareHorizontal size={14} /> Open as new tile
        </button>
      </div>
    </>
  );
}
