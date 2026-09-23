import { useRef, useState, type ReactNode } from "react";
import { ChevronDown, FolderOpen } from "lucide-react";
import { useUi } from "../stores/ui";
import { useSettings, type OpenTarget } from "../stores/settings";
import { useWorkspaces } from "../stores/workspace";
import { useNotifications } from "../stores/notifications";
import { openInEditor, revealPath, type EditorId } from "../api";
import { openFolderLabel } from "../platform";
import { FILE_MANAGER_LOGO } from "../lib/brand";
import { Menu } from "./Menu";
import vscodeLogo from "../assets/brand/vscode.svg";

// Cursor's mark is one colour, so it's drawn inline in the text colour: light on the dark theme,
// dark on the light one. As a fixed-colour image it vanished against one of the two.
function CursorMark() {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className="editor-mark">
      <path d="M11.503.131 1.891 5.678a.84.84 0 0 0-.42.726v11.188c0 .3.162.575.42.724l9.609 5.55a1 1 0 0 0 .998 0l9.61-5.55a.84.84 0 0 0 .42-.724V6.404a.84.84 0 0 0-.42-.726L12.497.131a1.01 1.01 0 0 0-.996 0M2.657 6.338h18.55c.263 0 .43.287.297.515L12.23 22.918c-.062.107-.229.064-.229-.06V12.335a.59.59 0 0 0-.295-.51l-9.11-5.257c-.109-.063-.064-.23.061-.23" />
    </svg>
  );
}

const EDITORS: { id: EditorId; name: string; logo: ReactNode }[] = [
  { id: "code", name: "VS Code", logo: <img src={vscodeLogo} alt="" /> },
  { id: "cursor", name: "Cursor", logo: <CursorMark /> },
];
// the OS file manager is a target like the editors; its label says which one (Explorer, Finder)
const FILES = {
  id: "files" as const,
  name: openFolderLabel,
  logo: FILE_MANAGER_LOGO ? <img src={FILE_MANAGER_LOGO} alt="" /> : <FolderOpen />,
};
const TARGETS = [...EDITORS, FILES];

// Titlebar split button: the chevron picks where the folder opens (an editor or the OS file
// manager), which only changes the button; clicking "Open" is what opens it.
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
  const current = TARGETS.find((t) => t.id === editor) ?? TARGETS[0];

  const pick = (id: OpenTarget) => {
    setOpen(false);
    setEditor(id);
  };
  const launch = (id: OpenTarget) => {
    void (id === "files" ? revealPath(folder) : openInEditor(folder, id)).catch((e) => {
      useNotifications.getState().add({ title: "Couldn't open folder", body: String(e) });
    });
  };

  return (
    <div className="tb-open" ref={anchor}>
      <button className="tb-action tb-open-main" title={current.id === "files" ? current.name : `Open in ${current.name}`} onClick={() => launch(current.id)}>
        {current.logo}
        Open
      </button>
      <button className={`tb-action tb-open-caret${open ? " open" : ""}`} title="Open in..." onClick={() => setOpen((o) => !o)}>
        <ChevronDown size={13} />
      </button>
      <Menu open={open} anchorRef={anchor} onClose={() => setOpen(false)} compact>
        {EDITORS.map((e) => (
          <button key={e.id} className={`pop-menu-item${e.id === current.id ? " on" : ""}`} onClick={() => pick(e.id)}>
            {e.logo}
            <span>{e.name}</span>
          </button>
        ))}
        <div className="pop-menu-sep" />
        <button className={`pop-menu-item${current.id === "files" ? " on" : ""}`} onClick={() => pick("files")}>
          {FILES.logo}
          <span>{FILES.name}</span>
        </button>
      </Menu>
    </div>
  );
}
