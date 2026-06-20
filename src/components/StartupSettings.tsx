import { useState } from "react";
import { useProjectConfigs, folderKey } from "../stores/projectConfig";
import { useWorkspaces } from "../stores/workspace";
import { pickFolder } from "../api";
import { ServicesEditor } from "./ServicesEditor";
import { Plus, X, Folder } from "lucide-react";

// Settings → Startup: manage per-folder startup services for any folder, even ones not open.
export function StartupSettings() {
  const configs = useProjectConfigs((s) => s.configs);
  const activeFolder = useWorkspaces((s) => {
    const w = s.workspaces.find((x) => x.id === s.activeId);
    return w && w.kind !== "open" ? folderKey(w.cwd) : "";
  });
  // folders the user explicitly removed this session — so the active-project auto-inject below
  // doesn't immediately bring them back
  const [dismissed, setDismissed] = useState<Set<string>>(() => new Set());
  const folders = Object.keys(configs);
  // always show the open project's folder, even before it has a config (unless just dismissed)
  const injected = activeFolder && !folders.includes(activeFolder) ? [activeFolder, ...folders] : folders;
  const list = injected.filter((f) => !dismissed.has(f));

  const [selected, setSelected] = useState(activeFolder || folders[0] || "");
  const sel = selected && list.includes(selected) ? selected : list[0] || "";

  const addFolder = async () => {
    const f = await pickFolder();
    if (!f) return;
    const k = folderKey(f);
    setDismissed((s) => {
      const n = new Set(s);
      n.delete(k);
      return n;
    });
    useProjectConfigs.getState().setConfig(f, {}); // create an entry so it appears in the list
    setSelected(k);
  };

  const removeFolder = (f: string) => {
    useProjectConfigs.getState().removeFolder(f);
    setDismissed((s) => new Set(s).add(f));
    if (sel === f) setSelected("");
  };

  return (
    <div className="startup-settings">
      <div className="su-bar">
        <span className="su-bar-label">Configured folders</span>
        <button className="btn" onClick={() => void addFolder()}>
          <Plus size={14} /> Add folder
        </button>
      </div>

      {list.length === 0 ? (
        <div className="svc-hint">
          No folders configured. Add one to set up the services that run when you open a project there.
        </div>
      ) : (
        <div className="su-list">
          {list.map((f) => (
            <div key={f} className={`su-item${f === sel ? " active" : ""}`}>
              <button className="su-pick" onClick={() => setSelected(f)} title={f}>
                <Folder size={13} />
                <span className="su-path">{f}</span>
              </button>
              <button
                className="su-del"
                title="Remove this folder's config"
                onClick={() => removeFolder(f)}
              >
                <X size={13} />
              </button>
            </div>
          ))}
        </div>
      )}

      {sel && (
        <div className="su-editor">
          <ServicesEditor folder={sel} name={sel} />
        </div>
      )}
    </div>
  );
}
