import { useRef, useState } from "react";
import { ChevronDown, FolderOpen } from "lucide-react";
import { useUi } from "../stores/ui";
import { useSettings } from "../stores/settings";
import { useWorkspaces } from "../stores/workspace";
import { useNotifications } from "../stores/notifications";
import { openInEditor, revealPath, type EditorId } from "../api";
import { openFolderLabel } from "../platform";
import { Menu } from "./Menu";
import vscodeLogo from "../assets/brand/vscode.svg";
import cursorLogo from "../assets/brand/cursor.svg";

const EDITORS: { id: EditorId; name: string; logo: string }[] = [
  { id: "code", name: "VS Code", logo: vscodeLogo },
  { id: "cursor", name: "Cursor", logo: cursorLogo },
];

// Titlebar split button: "Open" launches the current folder in the last editor you picked, and the
// chevron lists the other editors plus the OS file manager. Picking an editor makes it the default.
export function OpenInEditor() {
  const view = useUi((s) => s.view);
  const editor = useSettings((s) => s.editor);
  const setEditor = useSettings((s) => s.setEditor);
  const folder = useWorkspaces((s) => {
    const w = s.workspaces.find((x) => x.id === s.activeId);
    if (!w) return "";
    // an open space has no folder of its own; use the focused pane's
    return w.cwd || w.sessions.find((ss) => ss.id === s.focusedSessionId)?.cwd || "";
  });
  const [open, setOpen] = useState(false);
  const anchor = useRef<HTMLDivElement>(null);

  if (view !== "space" || !folder) return null;
  const current = EDITORS.find((e) => e.id === editor) ?? EDITORS[0];

  const launch = (id: EditorId) => {
    setOpen(false);
    setEditor(id);
    void openInEditor(folder, id).catch((e) => {
      useNotifications.getState().add({ title: "Couldn't open folder", body: String(e) });
    });
  };

  return (
    <div className="tb-open" ref={anchor}>
      <button className="tb-action tb-open-main" title={`Open in ${current.name}`} onClick={() => launch(current.id)}>
        <img src={current.logo} alt="" />
        Open
      </button>
      <button className={`tb-action tb-open-caret${open ? " open" : ""}`} title="Open in..." onClick={() => setOpen((o) => !o)}>
        <ChevronDown size={13} />
      </button>
      <Menu open={open} anchorRef={anchor} onClose={() => setOpen(false)} compact>
        {EDITORS.map((e) => (
          <button key={e.id} className={`pop-menu-item${e.id === current.id ? " on" : ""}`} onClick={() => launch(e.id)}>
            <img src={e.logo} alt="" />
            <span>{e.name}</span>
          </button>
        ))}
        <div className="pop-menu-sep" />
        <button
          className="pop-menu-item"
          onClick={() => {
            setOpen(false);
            void revealPath(folder).catch(() => {});
          }}
        >
          <FolderOpen />
          <span>{openFolderLabel}</span>
        </button>
      </Menu>
    </div>
  );
}
